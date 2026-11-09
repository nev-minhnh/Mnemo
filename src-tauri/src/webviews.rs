//! The window: Mnemo's own interface fills it ("shell"); each account is a child webview
//! laid over the content area, with its own profile folder (cookies, storage), so several
//! accounts of the same app stay separate. The writing popup is a small window of its own.

use crate::media;
use crate::services::{self, Nav};
use crate::settings::Account;
use crate::state::{AppState, lock};
use tauri::webview::{
    DownloadEvent, NewWindowResponse, PermissionKind, PermissionResponse, WebviewBuilder,
};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};

pub const MAIN: &str = "main";
pub const SHELL: &str = "shell";
pub const ASSIST: &str = "assist";
pub const SIDEBAR_W: f64 = 68.0;
pub const PANEL_W: f64 = 380.0;

pub fn label_for(account_id: &str) -> String {
    format!("acc-{account_id}")
}

pub fn account_of_label(label: &str) -> Option<&str> {
    label.strip_prefix("acc-")
}

pub fn open_external(app: &AppHandle, url: &str) {
    use tauri_plugin_opener::OpenerExt;
    if url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("mailto:")
        || url.starts_with("tel:")
    {
        if let Err(e) = app.opener().open_url(url, None::<&str>) {
            log::warn!("cannot open {url}: {e}");
        }
    }
}

/// Logical size of the window's content area.
fn inner_logical(window: &tauri::Window) -> (f64, f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or(LogicalSize::new(1280.0, 800.0));
    (size.width, size.height)
}

fn account_rect(
    app: &AppHandle,
    window: &tauri::Window,
) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let (w, h) = inner_logical(window);
    let panel = if *lock(&app.state::<AppState>().panel_open) {
        PANEL_W
    } else {
        0.0
    };
    let width = (w - SIDEBAR_W - panel).max(200.0);
    (
        LogicalPosition::new(SIDEBAR_W, 0.0),
        LogicalSize::new(width, h.max(200.0)),
    )
}

/// Puts the shell and the visible account where they belong (after a resize or a panel toggle).
pub fn layout(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN) else {
        return;
    };
    let (w, h) = inner_logical(&window);
    if let Some(shell) = app.get_webview(SHELL) {
        let _ = shell.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = shell.set_size(LogicalSize::new(w, h));
    }
    let visible = lock(&app.state::<AppState>().visible).clone();
    if let Some(id) = visible {
        if let Some(wv) = app.get_webview(&label_for(&id)) {
            let (pos, size) = account_rect(app, &window);
            let _ = wv.set_position(pos);
            let _ = wv.set_size(size);
        }
    }
}

fn on_title(app: &AppHandle, account_id: &str, title: &str) {
    let count = unread_from_title(title);
    let state = app.state::<AppState>();
    let changed = {
        let mut u = lock(&state.unread);
        u.insert(account_id.to_string(), count) != Some(count)
    };
    if changed {
        let _ = app.emit_to(
            SHELL,
            "unread",
            serde_json::json!({ "account": account_id, "count": count }),
        );
    }
}

/// "(3) WhatsApp", "Telegram (12)", "(1) Messenger | Facebook" → 3, 12, 1.
pub fn unread_from_title(title: &str) -> u32 {
    let re = regex::Regex::new(r"\((\d{1,5})\+?\)").expect("valid regex");
    re.captures(title)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0)
}

fn on_download(app: &AppHandle, account_id: &str, event: DownloadEvent<'_>) -> bool {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let Some(account) = settings.account(account_id).cloned() else {
        return true;
    };
    match event {
        DownloadEvent::Requested { url, destination } => {
            let dir = media::manual_downloads_dir(
                &settings.downloads_dir,
                account.service,
                &account.label,
            );
            if std::fs::create_dir_all(&dir).is_err() {
                return true; // the browser's default place still works
            }
            let suggested = destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty())
                .or_else(|| {
                    url.path_segments()
                        .and_then(|mut s| s.next_back().map(|x| x.to_string()))
                })
                .unwrap_or_else(|| "download".into());
            *destination =
                crate::util::unique_path(&dir, &crate::util::safe_component(&suggested, 120));
            true
        }
        DownloadEvent::Finished { path, success, .. } => {
            if let (true, Some(path)) = (success, path) {
                let app = app.clone();
                let account_id = account_id.to_string();
                std::thread::spawn(move || record_download(&app, &account_id, &path));
            }
            true
        }
        _ => true,
    }
}

fn record_download(app: &AppHandle, account_id: &str, path: &std::path::Path) {
    let state = app.state::<AppState>();
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    let sha = crate::util::sha256_hex(&data);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = crate::util::ext_of(&name);
    let kind = crate::rules::categorize(&ext, "").as_str();
    let chat_id = {
        let ctx = lock(&state.context).get(account_id).cloned();
        let settings = state.settings();
        match (ctx.and_then(|c| c.chat_key), settings.account(account_id)) {
            (Some(k), Some(a)) => state
                .with_archive(|ar| ar.find_chat(a.service.as_str(), account_id, &k))
                .ok()
                .flatten(),
            _ => None,
        }
    };
    let res = state.with_archive(|a| {
        a.add_media(&crate::archive::NewMedia {
            message_id: None,
            chat_id,
            kind,
            file_name: &name,
            mime: "",
            size: data.len() as i64,
            sha256: &sha,
            path,
            encrypted: false,
            origin: "download",
        })
    });
    if res.is_ok() {
        state.bump_stats(account_id, 0, 1, None);
        let _ = app.emit_to(
            SHELL,
            "download",
            serde_json::json!({ "account": account_id, "name": name, "path": path }),
        );
    }
}

/// Creates the account's webview if it doesn't exist yet.
pub fn ensure_account(app: &AppHandle, account: &Account) -> Result<Webview, String> {
    let label = label_for(&account.id);
    if let Some(w) = app.get_webview(&label) {
        return Ok(w);
    }
    let state = app.state::<AppState>();
    let def = services::def(account.service);
    let page_cfg = services::page_config(&state.config_dir, account.service);
    let profile = state.config_dir.join("profiles").join(&account.id);
    let window = app.get_window(MAIN).ok_or("Cửa sổ chính chưa sẵn sàng")?;
    let service = account.service;
    let (a1, a2, a3, a4) = (app.clone(), app.clone(), app.clone(), app.clone());
    let (id_title, id_dl) = (account.id.clone(), account.id.clone());
    let url = def.url.parse().map_err(|e| format!("{e}"))?;
    let builder = WebviewBuilder::new(&label, WebviewUrl::External(url))
        .data_directory(profile)
        .initialization_script(services::init_script(service, &account.id, &page_cfg))
        .on_navigation(
            move |url| match services::navigation(services::def(service), url) {
                Nav::Allow => true,
                Nav::OpenExternally => {
                    open_external(&a1, url.as_str());
                    false
                }
                Nav::Block => false,
            },
        )
        .on_new_window(move |url, _features| {
            match services::navigation(services::def(service), &url) {
                // Call windows and sign-in popups of the service itself open as popups.
                Nav::Allow => NewWindowResponse::Allow,
                Nav::OpenExternally => {
                    open_external(&a2, url.as_str());
                    NewWindowResponse::Deny
                }
                Nav::Block => NewWindowResponse::Deny,
            }
        })
        .on_document_title_changed(move |_wv, title| on_title(&a3, &id_title, &title))
        .on_download(move |_wv, event| on_download(&a4, &id_dl, event))
        .on_permission_request(|_wv, kind| match kind {
            // Calls, voice notes and message notifications.
            PermissionKind::Microphone
            | PermissionKind::Camera
            | PermissionKind::Notifications
            | PermissionKind::ClipboardRead => PermissionResponse::Allow,
            _ => PermissionResponse::Default,
        })
        .zoom_hotkeys_enabled(true)
        .devtools(true);
    let (pos, size) = account_rect(app, &window);
    let wv = window
        .add_child(builder, pos, size)
        .map_err(|e| format!("Không mở được {}: {e}", service.display()))?;
    Ok(wv)
}

/// Shows one account and hides the others.
pub fn show_account(app: &AppHandle, account_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let account = state
        .settings()
        .account(account_id)
        .cloned()
        .ok_or("Không tìm thấy tài khoản")?;
    let wv = ensure_account(app, &account)?;
    for other in app.webviews().values() {
        if other.label().starts_with("acc-") && other.label() != wv.label() {
            let _ = other.hide();
        }
    }
    *lock(&state.visible) = Some(account_id.to_string());
    layout(app);
    let _ = wv.show();
    let _ = wv.set_focus();
    let _ = app.emit_to(
        SHELL,
        "visible",
        serde_json::json!({ "account": account_id }),
    );
    Ok(())
}

/// Hides every account so Mnemo's own pages (archive, settings) show.
pub fn hide_accounts(app: &AppHandle) {
    for wv in app.webviews().values() {
        if wv.label().starts_with("acc-") {
            let _ = wv.hide();
        }
    }
    *lock(&app.state::<AppState>().visible) = None;
    let _ = app.emit_to(SHELL, "visible", serde_json::json!({ "account": null }));
}

/// Closes an account's page (frees its memory); it opens again when selected.
pub fn close_account(app: &AppHandle, account_id: &str) {
    if let Some(wv) = app.get_webview(&label_for(account_id)) {
        let _ = wv.close();
    }
    let state = app.state::<AppState>();
    let mut visible = lock(&state.visible);
    if visible.as_deref() == Some(account_id) {
        *visible = None;
    }
}

/// Positions the writing popup under the caret (or above it near the screen's bottom) and shows it.
pub fn show_assist(
    app: &AppHandle,
    account_id: &str,
    rect: (f64, f64, f64, f64),
) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;
    let popup = match app.get_webview_window(ASSIST) {
        Some(p) => p,
        None => {
            tauri::WebviewWindowBuilder::new(app, ASSIST, WebviewUrl::App("assist.html".into()))
                .title("Mnemo – trợ lý viết")
                .inner_size(440.0, 560.0)
                .decorations(false)
                .resizable(true)
                .skip_taskbar(true)
                .always_on_top(true)
                .visible(false)
                .shadow(true)
                .build()
                .map_err(|e| e.to_string())?
        }
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let (x, y, _w, h) = rect;
    let panel_x = if lock(&app.state::<AppState>().visible).as_deref() == Some(account_id) {
        SIDEBAR_W
    } else {
        0.0
    };
    let popup_w = 440.0 * scale;
    let popup_h = 560.0 * scale;
    let mut px = origin.x as f64 + (panel_x + x) * scale - 24.0 * scale;
    let mut py = origin.y as f64 + (y + h + 8.0) * scale;
    if let Ok(Some(m)) = window.current_monitor() {
        let mp = m.position();
        let ms = m.size();
        let right = mp.x as f64 + ms.width as f64;
        let bottom = mp.y as f64 + ms.height as f64;
        if px + popup_w > right {
            px = right - popup_w - 8.0;
        }
        if py + popup_h > bottom {
            // Not enough room below the caret: open above it.
            py = (origin.y as f64 + (y - 8.0) * scale - popup_h).max(mp.y as f64 + 8.0);
        }
        px = px.max(mp.x as f64 + 8.0);
    }
    let _ = popup.set_position(tauri::PhysicalPosition::new(
        px.round() as i32,
        py.round() as i32,
    ));
    let _ = popup.show();
    let _ = popup.set_focus();
    Ok(())
}

pub fn hide_assist(app: &AppHandle) {
    if let Some(p) = app.get_webview_window(ASSIST) {
        let _ = p.hide();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unread_titles() {
        assert_eq!(super::unread_from_title("(3) WhatsApp"), 3);
        assert_eq!(super::unread_from_title("Telegram (12)"), 12);
        assert_eq!(super::unread_from_title("Messenger | Facebook"), 0);
        assert_eq!(super::unread_from_title("(99+) Zalo"), 99);
    }
}
