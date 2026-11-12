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
/// Default sidebar width until the shell reports its own.
pub const SIDEBAR_W: f64 = 248.0;
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
    let state = app.state::<AppState>();
    let panel = if *lock(&state.panel_open) {
        PANEL_W
    } else {
        0.0
    };
    // The bar's thickness: its width on the left/right, its height on the top/bottom.
    let bar = sidebar_w(app);
    let pos = lock(&state.sidebar_pos).clone();
    let (x, y, width, height) = match pos.as_str() {
        "right" => (0.0, 0.0, w - bar - panel, h),
        "top" => (0.0, bar, w - panel, h - bar),
        "bottom" => (0.0, 0.0, w - panel, h - bar),
        _ => (bar, 0.0, w - bar - panel, h),
    };
    (
        LogicalPosition::new(x, y),
        LogicalSize::new(width.max(200.0), height.max(200.0)),
    )
}

/// Where an account that isn't shown waits: just outside the window, at full size.
/// Hiding a WebView2 tells the page it went to the background, and apps such as WhatsApp
/// then drop their connection and load again from scratch when you come back; a parked
/// page keeps running exactly as it was, so switching accounts is instant.
fn parked(size: &LogicalSize<f64>) -> LogicalPosition<f64> {
    LogicalPosition::new(-(size.width + 400.0), 0.0)
}

/// Asks WebView2 to keep a page that isn't on screen small: it releases caches and memory
/// it can rebuild, while the page keeps running (messages and notifications still arrive).
pub fn memory_level(wv: &Webview, low: bool) {
    #[cfg(windows)]
    {
        let _ = wv.with_webview(move |p| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, ICoreWebView2_19,
            };
            use windows::core::Interface;
            if let Ok(core) = p.controller().CoreWebView2() {
                if let Ok(w) = core.cast::<ICoreWebView2_19>() {
                    let _ = w.SetMemoryUsageTargetLevel(if low {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                    } else {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                    });
                }
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (wv, low);
}

/// Moves an account's page out of sight without pausing it.
pub fn park(app: &AppHandle, wv: &Webview) {
    if let Some(window) = app.get_window(MAIN) {
        let (_, size) = account_rect(app, &window);
        place(wv, parked(&size), size, true);
    }
}

/// Puts the shell and the visible account where they belong (after a resize or a panel toggle).
pub fn layout(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN) else {
        return;
    };
    let (w, h) = inner_logical(&window);
    if let Some(shell) = app.get_webview(SHELL) {
        place(
            &shell,
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(w, h),
            false,
        );
    }
    let visible = lock(&app.state::<AppState>().visible)
        .as_deref()
        .map(label_for);
    let (pos, size) = account_rect(app, &window);
    // Mnemo in the tray or minimised: every page goes to low memory, the shown one too.
    let away = !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false);
    for wv in app.webviews().values() {
        if !wv.label().starts_with("acc-") {
            continue;
        }
        // A hidden page header sits just above the window instead of leaving a gap.
        let crop = wv
            .label()
            .strip_prefix("acc-")
            .and_then(|id| lock(&app.state::<AppState>().crop_top).get(id).copied())
            .unwrap_or(0.0);
        let shown = visible.as_deref() == Some(wv.label());
        let at = if shown {
            LogicalPosition::new(pos.x, pos.y - crop)
        } else {
            parked(&size)
        };
        place(
            wv,
            at,
            LogicalSize::new(size.width, size.height + crop),
            away || !shown,
        );
    }
}

/// Last place, size and memory level given to each page. Moving or resizing a WebView2 makes
/// its page lay out again, so nothing is sent when nothing changed (layout runs on every
/// focus and resize of the window).
static PLACED: std::sync::Mutex<Option<std::collections::HashMap<String, [i64; 5]>>> =
    std::sync::Mutex::new(None);

fn place(wv: &Webview, at: LogicalPosition<f64>, size: LogicalSize<f64>, low: bool) {
    let key = [
        at.x.round() as i64,
        at.y.round() as i64,
        size.width.round() as i64,
        size.height.round() as i64,
        low as i64,
    ];
    let mut g = PLACED.lock().unwrap_or_else(|e| e.into_inner());
    let before = g
        .get_or_insert_with(std::collections::HashMap::new)
        .insert(wv.label().to_string(), key);
    drop(g);
    let before = before.unwrap_or([i64::MIN; 5]);
    if before[2..4] != key[2..4] {
        let _ = wv.set_size(size);
    }
    if before[0..2] != key[0..2] {
        let _ = wv.set_position(at);
    }
    if before[4] != key[4] {
        memory_level(wv, low);
    }
}

/// A page that was closed (or recreated) has no remembered place.
pub fn forget_place(label: &str) {
    if let Some(m) = PLACED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(label);
    }
}

pub fn sidebar_w(app: &AppHandle) -> f64 {
    *lock(&app.state::<AppState>().sidebar_w)
}

fn on_title(app: &AppHandle, account_id: &str, title: &str) {
    // Pages that count their own unread messages (Telegram, WhatsApp) are not read from
    // the title: Telegram puts no count there and WhatsApp counts chats, not messages.
    if lock(&app.state::<AppState>().page_unread).contains(account_id) {
        return;
    }
    set_unread(app, account_id, unread_from_title(title));
}

/// Records an account's unread count and tells everything that shows it.
pub fn set_unread(app: &AppHandle, account_id: &str, count: u32) {
    let state = app.state::<AppState>();
    let before = lock(&state.unread).insert(account_id.to_string(), count);
    let changed = before != Some(count);
    if let Some(b) = before {
        if count > b {
            crate::notify::unread_rose(app, account_id, b, count);
        }
    }
    if changed {
        crate::notify::update_taskbar(app);
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
    // A new page: whatever was remembered for an earlier one with this label is stale.
    forget_place(&label);
    let state = app.state::<AppState>();
    let def = services::def(account.service);
    let page_cfg = services::page_config(&state.config_dir, account.service);
    let profile = crate::profiles::dir(&state.config_dir, account);
    let window = app.get_window(MAIN).ok_or("Cửa sổ chính chưa sẵn sàng")?;
    let service = account.service;
    let (a1, a2, a3, a4) = (app.clone(), app.clone(), app.clone(), app.clone());
    let (id_title, id_dl) = (account.id.clone(), account.id.clone());
    let url = def.url.parse().map_err(|e| format!("{e}"))?;
    // End of the current sign-in window, shared by page navigations and popups.
    let sign_in = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (si1, si2) = (sign_in.clone(), sign_in);
    let builder = WebviewBuilder::new(&label, WebviewUrl::External(url))
        .data_directory(profile)
        .initialization_script(services::init_script(service, &account.id, &page_cfg))
        .on_navigation(move |url| {
            match services::navigation_signin(
                services::def(service),
                url,
                &si1,
                crate::util::now_ms() as u64,
            ) {
                Nav::Allow => true,
                Nav::OpenExternally => {
                    open_external(&a1, url.as_str());
                    false
                }
                Nav::Block => false,
            }
        })
        .on_new_window(move |url, _features| {
            match services::navigation_signin(
                services::def(service),
                &url,
                &si2,
                crate::util::now_ms() as u64,
            ) {
                // Call windows and sign-in popups (the service's own, or Google's
                // "confirm it's you") open as popups in the account's profile, so the
                // result goes back to the page that asked for it.
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
            // Notifications go through Mnemo (the page script hands them over), so the
            // browser's own are refused to avoid showing everything twice.
            PermissionKind::Notifications => PermissionResponse::Deny,
            PermissionKind::Microphone | PermissionKind::Camera | PermissionKind::ClipboardRead => {
                PermissionResponse::Allow
            }
            _ => PermissionResponse::Default,
        })
        .zoom_hotkeys_enabled(true)
        // Tauri's own defaults, plus: never slow down or freeze an app's page because it
        // isn't on screen, so messages, calls and notifications keep arriving live.
        // Every account sharing a folder must pass the same arguments.
        .additional_browser_args(BROWSER_ARGS)
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
    if account.disabled {
        return Err("Tài khoản đang tạm tắt. Bật lại trong trang chủ.".into());
    }
    let wv = ensure_account(app, &account)?;
    *lock(&state.visible) = Some(account_id.to_string());
    // The others are parked, not hidden, so their pages keep running.
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
    *lock(&app.state::<AppState>().visible) = None;
    layout(app);
    if let Some(shell) = app.get_webview(SHELL) {
        let _ = shell.set_focus();
    }
    let _ = app.emit_to(SHELL, "visible", serde_json::json!({ "account": null }));
}

/// Closes an account's page (frees its memory); it opens again when selected.
pub fn close_account(app: &AppHandle, account_id: &str) {
    if let Some(wv) = app.get_webview(&label_for(account_id)) {
        let _ = wv.close();
    }
    forget_place(&label_for(account_id));
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
                .additional_browser_args(BROWSER_ARGS)
                .data_directory(own_dir(app))
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
    // Where the account's page starts inside the window (its rect is page-relative).
    let (page_x, page_y) = if lock(&app.state::<AppState>().visible).as_deref() == Some(account_id)
    {
        let (p, _) = account_rect(app, &window);
        (p.x, p.y)
    } else {
        (0.0, 0.0)
    };
    let popup_w = 440.0 * scale;
    let popup_h = 560.0 * scale;
    let mut px = origin.x as f64 + (page_x + x) * scale - 24.0 * scale;
    let mut py = origin.y as f64 + (page_y + y + h + 8.0) * scale;
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

// ---------------------------------------------------------------- Add account popover

/// Arguments of every page, the accounts' and Mnemo's own (pages in one data folder run in
/// one browser and must all pass exactly the same ones):
/// - Tauri's own defaults;
/// - never slow down or freeze a page because it isn't on screen, so messages, calls and
///   notifications keep arriving live.
/// Hardware acceleration stays on (the browser's default): scrolling is smooth and uses
/// less battery than drawing with the CPU, for one GPU process per browser.
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --disable-background-timer-throttling --disable-renderer-backgrounding \
     --disable-backgrounding-occluded-windows";

/// The data folder of Mnemo's own windows (the bar, notifications, popovers): the accounts'
/// folder holding the most accounts, so the interface runs inside that browser instead of
/// starting one of its own. Chosen once per run.
pub fn own_dir(app: &AppHandle) -> std::path::PathBuf {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let state = app.state::<AppState>();
        crate::profiles::busiest(&state.config_dir, &state.settings())
    })
    .clone()
}

pub const PICKER: &str = "picker";
const PICKER_W: f64 = 584.0;
const PICKER_H: f64 = 330.0;
/// Where the popover was last placed (its page asks when it first loads).
static PICKER_PLACE: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);
/// When the popover last closed: clicking + while it is open first takes the focus away
/// (which closes it), and that same click must not open it again.
static PICKER_CLOSED: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

fn picker_hidden(app: &AppHandle) {
    *PICKER_CLOSED.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
    let _ = app.emit_to(SHELL, "picker", false);
}

#[derive(serde::Deserialize, Clone, Copy, Debug)]
pub struct ButtonRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Opens the Add account popover next to the sidebar's + button. It is its own small window,
/// so it shows over the open account's page instead of sending you back to the home page.
#[tauri::command]
pub async fn open_picker(app: AppHandle, rect: ButtonRect) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;
    // The + button toggles: a second click closes the popover.
    if let Some(p) = app.get_webview_window(PICKER) {
        if p.is_visible().unwrap_or(false) {
            let _ = p.hide();
            picker_hidden(&app);
            return Ok(());
        }
    }
    let just_closed = PICKER_CLOSED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(350));
    if just_closed {
        return Ok(());
    }
    let picker = match app.get_webview_window(PICKER) {
        Some(p) => p,
        None => {
            let p = tauri::WebviewWindowBuilder::new(
                &app,
                PICKER,
                WebviewUrl::App("picker.html".into()),
            )
            .title("Mnemo – thêm tài khoản")
            .additional_browser_args(BROWSER_ARGS)
            .data_directory(own_dir(&app))
            .inner_size(PICKER_W, PICKER_H)
            .decorations(false)
            .transparent(true)
            .resizable(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .shadow(false)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
            let handle = app.clone();
            p.on_window_event(move |e| {
                if let tauri::WindowEvent::Focused(false) = e {
                    if let Some(w) = handle.get_webview_window(PICKER) {
                        if w.is_visible().unwrap_or(false) {
                            let _ = w.hide();
                            picker_hidden(&handle);
                        }
                    }
                }
            });
            p
        }
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let (ox, oy) = (origin.x as f64 / scale, origin.y as f64 / scale);
    let pos = lock(&app.state::<AppState>().sidebar_pos).clone();
    let (bx, by) = (ox + rect.x, oy + rect.y);
    let (cx, cy) = (bx + rect.width / 2.0, by + rect.height / 2.0);
    // Beside the button, on the side away from the window edge the bar is on.
    let (mut x, mut y) = match pos.as_str() {
        "right" => (bx - PICKER_W - 6.0, cy - 40.0),
        "top" => (cx - 40.0, by + rect.height + 6.0),
        "bottom" => (cx - 40.0, by - PICKER_H - 6.0),
        _ => (bx + rect.width + 6.0, cy - 40.0),
    };
    if let Ok(Some(m)) = window.current_monitor() {
        let (mx, my) = (m.position().x as f64 / scale, m.position().y as f64 / scale);
        let (mw, mh) = (
            m.size().width as f64 / scale,
            m.size().height as f64 / scale,
        );
        x = x.clamp(mx + 4.0, mx + mw - PICKER_W - 4.0);
        y = y.clamp(my + 4.0, my + mh - PICKER_H - 4.0);
    }
    // The little arrow points at the button wherever the box ended up.
    let arrow = match pos.as_str() {
        "top" | "bottom" => (cx - x).clamp(24.0, PICKER_W - 24.0),
        _ => (cy - y).clamp(24.0, PICKER_H - 24.0),
    };
    let _ = picker.set_position(tauri::LogicalPosition::new(x, y));
    let _ = picker.set_size(LogicalSize::new(PICKER_W, PICKER_H));
    let place = serde_json::json!({ "side": pos, "arrow": arrow });
    *PICKER_PLACE.lock().unwrap_or_else(|e| e.into_inner()) = Some(place.clone());
    let _ = app.emit_to(PICKER, "picker-open", place);
    let _ = picker.show();
    let _ = picker.set_focus();
    let _ = app.emit_to(SHELL, "picker", true);
    Ok(())
}

/// What the popover shows: the apps with their icons, and the accounts whose sign-in a new
/// Instagram or Threads account can reuse.
#[tauri::command]
pub fn picker_state(state: tauri::State<'_, AppState>) -> serde_json::Value {
    let s = state.settings();
    let services: Vec<serde_json::Value> = crate::settings::ServiceId::ALL
        .iter()
        .map(|id| serde_json::json!({ "id": id.as_str(), "name": id.display() }))
        .collect();
    serde_json::json!({
        "services": services,
        "accounts": s.accounts.iter().map(|a| serde_json::json!({
            "id": a.id, "service": a.service.as_str(), "label": a.label, "disabled": a.disabled,
        })).collect::<Vec<_>>(),
        "icons": *lock(&state.icons),
        "avatars": *lock(&state.avatars),
        "language": s.language,
        "theme": s.theme,
        "place": PICKER_PLACE.lock().unwrap_or_else(|e| e.into_inner()).clone(),
    })
}

#[tauri::command]
pub fn picker_close(app: AppHandle) {
    if let Some(w) = app.get_webview_window(PICKER) {
        let _ = w.hide();
        picker_hidden(&app);
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
