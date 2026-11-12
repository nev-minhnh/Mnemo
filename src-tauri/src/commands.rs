//! Commands for Mnemo's own interface (the shell). Pages of the apps can't call these
//! (see capabilities/shell.json and capabilities/services.json).

use crate::archive::{Archive, ChatRow, MediaRow, MessageRow, SearchHit};
use crate::importers::{self, ImportKind, ImportReport};
use crate::settings::{Account, ProviderConfig, ProviderKind, ServiceId, Settings};
use crate::state::{AppState, lock};
use crate::webviews;
use base64::Engine;
use serde_json::{Value, json};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub fn save(state: &AppState, s: &Settings) -> Result<(), String> {
    s.save(&state.config_dir)
        .map_err(|e| format!("Không lưu được cài đặt: {e}"))
}

#[tauri::command]
pub fn get_state(app: AppHandle, state: State<'_, AppState>) -> Value {
    let s = state.settings();
    let archive = state.with_archive(|a| a.stats());
    let services: Vec<Value> = ServiceId::ALL
        .iter()
        .map(|id| json!({ "id": id.as_str(), "name": id.display(), "url": crate::services::def(*id).url }))
        .collect();
    json!({
        "version": app.package_info().version.to_string(),
        "settings": s,
        "services": services,
        "unread": *lock(&state.unread),
        "stats": *lock(&state.stats),
        "context": *lock(&state.context),
        "visible": *lock(&state.visible),
        "panelOpen": *lock(&state.panel_open),
        "archive": match archive {
            Ok(st) => json!({ "ok": true, "stats": st }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "configDir": state.config_dir,
        "providers": crate::ai::ai_providers(state.clone()),
        "avatars": *lock(&state.avatars),
        "icons": *lock(&state.icons),
    })
}

/// Saves settings from the interface. Folders change through their own commands.
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    let mut cur = lock(&state.settings);
    let mut next = settings;
    next.archive_dir = cur.archive_dir.clone();
    next.accounts = cur.accounts.clone();
    next.start_with_windows = cur.start_with_windows;
    next.share_processes = cur.share_processes;
    if next.downloads_dir.as_os_str().is_empty() {
        next.downloads_dir = cur.downloads_dir.clone();
    }
    save(&state, &next)?;
    let lang_changed = cur.language != next.language;
    let tray_changed = cur.close_to_tray != next.close_to_tray;
    *cur = next;
    drop(cur);
    if lang_changed || tray_changed {
        crate::refresh_tray_menu(&app);
    }
    crate::notify::update_taskbar(&app);
    Ok(())
}

#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
    service: String,
    label: String,
    share_with: Option<String>,
) -> Result<Account, String> {
    let service = ServiceId::parse(&service).ok_or("Dịch vụ không hợp lệ")?;
    let account = {
        let mut s = lock(&state.settings);
        let n = s.accounts.iter().filter(|a| a.service == service).count() + 1;
        let label = if label.trim().is_empty() {
            if n == 1 {
                service.display().to_string()
            } else {
                format!("{} {n}", service.display())
            }
        } else {
            crate::util::truncate_chars(label.trim(), 60)
        };
        let account = Account {
            id: crate::util::random_id(6),
            service,
            label,
            ..Account::default()
        };
        s.accounts.push(account.clone());
        // "Continue with" a Facebook/Instagram account already in Mnemo: same browser data.
        let linked = match share_with.filter(|w| !w.is_empty()) {
            Some(with) => match crate::profiles::link(&mut s, &account.id, &with) {
                Ok(()) => true,
                Err(e) => {
                    s.accounts.retain(|a| a.id != account.id);
                    return Err(e);
                }
            },
            None => false,
        };
        if s.share_processes && !linked {
            crate::profiles::assign(&mut s, &account.id);
        }
        save(&state, &s)?;
        s.account(&account.id).cloned().unwrap_or(account)
    };
    // Show the new account in the sidebar right away; the page itself may take a moment.
    let _ = app.emit_to(webviews::SHELL, "accounts", &account.id);
    webviews::show_account(&app, &account.id)?;
    Ok(account)
}

#[tauri::command]
pub fn update_account(state: State<'_, AppState>, account: Account) -> Result<(), String> {
    let mut s = lock(&state.settings);
    let slot = s
        .accounts
        .iter_mut()
        .find(|a| a.id == account.id)
        .ok_or("Không tìm thấy tài khoản")?;
    let service = slot.service;
    *slot = Account {
        service,
        label: crate::util::truncate_chars(account.label.trim(), 60),
        ..account
    };
    save(&state, &s)
}

#[tauri::command]
pub fn move_account(state: State<'_, AppState>, id: String, delta: i32) -> Result<(), String> {
    let mut s = lock(&state.settings);
    let i = s
        .accounts
        .iter()
        .position(|a| a.id == id)
        .ok_or("Không tìm thấy tài khoản")?;
    let j = (i as i64 + delta as i64).clamp(0, s.accounts.len() as i64 - 1) as usize;
    s.accounts.swap(i, j);
    save(&state, &s)
}

/// Puts the accounts in the order the person dragged them to in the sidebar.
/// Ids it doesn't know are ignored; accounts it leaves out keep their place at the end.
#[tauri::command]
pub fn reorder_accounts(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    let mut s = lock(&state.settings);
    let mut rest = std::mem::take(&mut s.accounts);
    let mut ordered = Vec::with_capacity(rest.len());
    for id in &ids {
        if let Some(i) = rest.iter().position(|a| &a.id == id) {
            ordered.push(rest.remove(i));
        }
    }
    ordered.extend(rest);
    s.accounts = ordered;
    save(&state, &s)
}

/// Removes an account. Its archived messages stay; `wipe_profile` also signs it out
/// for good by deleting its browser data.
#[tauri::command]
pub fn remove_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    wipe_profile: bool,
) -> Result<(), String> {
    webviews::close_account(&app, &id);
    // A folder another account still runs in is never deleted.
    let wipe_dir = {
        let mut s = lock(&state.settings);
        let dir = s.account(&id).and_then(|a| {
            (!crate::profiles::shared_with_others(&s, a))
                .then(|| crate::profiles::dir(&state.config_dir, a))
        });
        s.accounts.retain(|a| a.id != id);
        save(&state, &s)?;
        dir
    };
    lock(&state.unread).remove(&id);
    lock(&state.avatars).remove(&id);
    let _ = std::fs::remove_file(crate::capture::avatar_path(&state.config_dir, &id));
    let _ = std::fs::remove_file(
        state
            .config_dir
            .join("avatars")
            .join(format!("{id}.manual")),
    );
    crate::notify::update_taskbar(&app);
    if let (true, Some(dir)) = (wipe_profile, wipe_dir) {
        if dir == webviews::own_dir(&app) {
            // Mnemo's own windows run in this folder: it goes at the next start.
            crate::profiles::delete_later(&dir);
            return Ok(());
        }
        // The browser process may still hold files for a moment after closing.
        std::thread::spawn(move || {
            for _ in 0..20 {
                if std::fs::remove_dir_all(&dir).is_ok() || !dir.exists() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn show_account(app: AppHandle, id: String) -> Result<(), String> {
    webviews::show_account(&app, &id)
}

#[tauri::command]
pub fn show_page(app: AppHandle) {
    webviews::hide_accounts(&app);
}

#[tauri::command]
pub async fn reload_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    hard: Option<bool>,
) -> Result<(), String> {
    let label = webviews::label_for(&id);
    if !hard.unwrap_or(false) {
        let wv = app.get_webview(&label).ok_or("Tài khoản chưa mở")?;
        return wv.reload().map_err(|e| e.to_string());
    }
    // A page that hangs may not answer a reload: close it and open a new one. The sign-in
    // is on disk, so it comes back signed in.
    let account = state
        .settings()
        .account(&id)
        .cloned()
        .ok_or("Không tìm thấy tài khoản")?;
    let was_visible = lock(&state.visible).as_deref() == Some(id.as_str());
    webviews::close_account(&app, &id);
    lock(&state.page_unread).remove(&id);
    lock(&state.writing).remove(&id);
    for _ in 0..40 {
        if app.get_webview(&label).is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if was_visible {
        webviews::show_account(&app, &id)
    } else {
        let wv = webviews::ensure_account(&app, &account)?;
        webviews::park(&app, &wv);
        Ok(())
    }
}

#[tauri::command]
pub fn sleep_account(app: AppHandle, id: String) {
    webviews::close_account(&app, &id);
    let _ = app.emit_to(webviews::SHELL, "visible", json!({ "account": null }));
}

#[tauri::command]
pub fn open_devtools(app: AppHandle, id: String) -> Result<(), String> {
    let wv = app
        .get_webview(&webviews::label_for(&id))
        .ok_or("Tài khoản chưa mở")?;
    wv.open_devtools();
    Ok(())
}

/// The shell reports its sidebar width and whether the AI panel is open.
#[tauri::command]
pub fn set_layout(
    app: AppHandle,
    state: State<'_, AppState>,
    sidebar: f64,
    panel: bool,
    pos: Option<String>,
) {
    *lock(&state.sidebar_w) = sidebar.clamp(56.0, 400.0);
    if let Some(p) = pos.filter(|p| ["left", "right", "top", "bottom"].contains(&p.as_str())) {
        *lock(&state.sidebar_pos) = p;
    }
    *lock(&state.panel_open) = panel;
    webviews::layout(&app);
}

/// Sets an account's avatar from an image file the person picked (`None` = back to automatic).
#[tauri::command]
pub fn set_account_avatar(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    path: Option<String>,
) -> Result<(), String> {
    let manual = state
        .config_dir
        .join("avatars")
        .join(format!("{id}.manual"));
    match path {
        Some(p) => {
            let bytes = std::fs::read(&p).map_err(|e| format!("Không đọc được ảnh: {e}"))?;
            let data = crate::capture::image_data_url(&bytes, "")
                .ok_or("Ảnh phải là PNG, JPG, WEBP hoặc GIF, nhỏ hơn 1,5 MB")?;
            crate::capture::set_avatar(&app, &state, &id, Some(data))?;
            std::fs::write(&manual, b"").map_err(|e| e.to_string())
        }
        None => {
            let _ = std::fs::remove_file(&manual);
            crate::capture::set_avatar(&app, &state, &id, None)
        }
    }
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle, title: Option<String>) -> Option<String> {
    let mut d = app.dialog().file();
    if let Some(t) = title {
        d = d.set_title(t);
    }
    d.blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn pick_file(
    app: AppHandle,
    title: Option<String>,
    extensions: Vec<String>,
) -> Option<String> {
    let mut d = app.dialog().file();
    if let Some(t) = title {
        d = d.set_title(t);
    }
    if !extensions.is_empty() {
        let exts: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();
        d = d.add_filter("Tệp", &exts);
    }
    d.blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Moves the archive to another folder: copies `mnemo.db` there (or opens the archive
/// already there with the current key), then switches. The old copy is left in place.
#[tauri::command]
pub fn set_archive_dir(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let new_dir = PathBuf::from(path.trim());
    if !new_dir.is_absolute() {
        return Err("Hãy chọn một thư mục đầy đủ".into());
    }
    std::fs::create_dir_all(&new_dir)
        .map_err(|e| format!("Không tạo được {}: {e}", new_dir.display()))?;
    let key = state.key()?;
    let old_dir = lock(&state.settings).archive_dir.clone();
    if new_dir == old_dir {
        return Ok("Không có gì thay đổi".into());
    }
    let target = Archive::db_path(&new_dir);
    let mut archive = lock(&state.archive);
    let message = if target.exists() {
        let opened = Archive::open(&new_dir, &key)?;
        *archive = Some(opened);
        format!("Đã chuyển sang kho có sẵn ở {}", new_dir.display())
    } else {
        if let Some(a) = archive.as_ref() {
            a.checkpoint()?;
        }
        *archive = None;
        std::fs::copy(Archive::db_path(&old_dir), &target)
            .map_err(|e| format!("Không chép được kho: {e}"))?;
        match Archive::open(&new_dir, &key) {
            Ok(a) => *archive = Some(a),
            Err(e) => {
                *archive = Archive::open(&old_dir, &key).ok();
                return Err(e);
            }
        }
        format!(
            "Đã chuyển kho sang {}. Bản cũ vẫn còn ở {}, bạn có thể tự xóa.",
            new_dir.display(),
            old_dir.display()
        )
    };
    *lock(&state.archive_error) = None;
    let mut s = lock(&state.settings);
    s.archive_dir = new_dir;
    save(&state, &s)?;
    Ok(message)
}

#[tauri::command]
pub fn set_downloads_dir(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let dir = PathBuf::from(path.trim());
    if !dir.is_absolute() {
        return Err("Hãy chọn một thư mục đầy đủ".into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("Không tạo được {}: {e}", dir.display()))?;
    let mut s = lock(&state.settings);
    s.downloads_dir = dir;
    save(&state, &s)
}

#[tauri::command]
pub fn list_chats(
    state: State<'_, AppState>,
    service: Option<String>,
    account: Option<String>,
    query: Option<String>,
) -> Result<Vec<ChatRow>, String> {
    state.with_archive(|a| {
        a.list_chats(
            service.as_deref(),
            account.as_deref(),
            query.as_deref(),
            500,
        )
    })
}

#[tauri::command]
pub fn chat_messages(
    state: State<'_, AppState>,
    chat_id: i64,
    before_ord: Option<i64>,
    before_id: Option<i64>,
    around: Option<i64>,
) -> Result<Vec<MessageRow>, String> {
    state.with_archive(|a| match around {
        Some(mid) => a.messages_around(chat_id, mid, 60),
        None => a.messages(chat_id, before_ord.zip(before_id), 120),
    })
}

#[tauri::command]
pub fn search_messages(
    state: State<'_, AppState>,
    query: String,
    service: Option<String>,
    offset: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    state.with_archive(|a| a.search(&query, service.as_deref(), 100, offset.unwrap_or(0)))
}

#[tauri::command]
pub fn mark_seen(state: State<'_, AppState>, chat_id: i64) -> Result<(), String> {
    state.with_archive(|a| a.mark_all_seen(chat_id))
}

/// The archived chat matching what an account's page shows right now.
#[tauri::command]
pub fn live_chat(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Option<ChatRow>, String> {
    let ctx = lock(&state.context).get(&account_id).cloned();
    let s = state.settings();
    let Some(account) = s.account(&account_id) else {
        return Ok(None);
    };
    let Some(key) = ctx.and_then(|c| c.chat_key) else {
        return Ok(None);
    };
    state.with_archive(
        |a| match a.find_chat(account.service.as_str(), &account_id, &key)? {
            Some(id) => a.chat(id),
            None => Ok(None),
        },
    )
}

#[tauri::command]
pub fn list_media(
    state: State<'_, AppState>,
    offset: Option<i64>,
) -> Result<Vec<MediaRow>, String> {
    state.with_archive(|a| a.media_list(200, offset.unwrap_or(0)))
}

/// A small image as a data URL for previews (decrypted in memory when needed).
#[tauri::command]
pub async fn media_preview(state: State<'_, AppState>, id: i64) -> Result<Option<String>, String> {
    let m = state
        .with_archive(|a| a.media_get(id))?
        .ok_or("Không tìm thấy tệp")?;
    if m.kind != "image" || m.size > 8 * 1024 * 1024 {
        return Ok(None);
    }
    let key = state.key()?;
    let data = crate::media::read_plain(std::path::Path::new(&m.path), &key)?;
    let ext = crate::util::ext_of(m.file_name.trim_end_matches(".mnemo"));
    let mime = match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    };
    Ok(Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    )))
}

/// Opens a saved file with its default app. Encrypted files are decrypted to a private
/// temporary folder first (emptied when Mnemo starts).
#[tauri::command]
pub fn open_media(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let m = state
        .with_archive(|a| a.media_get(id))?
        .ok_or("Không tìm thấy tệp")?;
    let path = PathBuf::from(&m.path);
    let target = if m.encrypted {
        let key = state.key()?;
        let data = crate::media::read_plain(&path, &key)?;
        let dir = state.config_dir.join("temp-open");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let out = crate::util::unique_path(
            &dir,
            &crate::util::safe_component(m.file_name.trim_end_matches(".mnemo"), 120),
        );
        std::fs::write(&out, data).map_err(|e| e.to_string())?;
        out
    } else {
        path
    };
    app.opener()
        .open_path(target.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_media(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let m = state
        .with_archive(|a| a.media_get(id))?
        .ok_or("Không tìm thấy tệp")?;
    app.opener()
        .reveal_item_in_dir(&m.path)
        .map_err(|e| e.to_string())
}

/// Opens one of Mnemo's folders in Explorer.
#[tauri::command]
pub fn open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    which: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let s = state.settings();
    let dir = match which.as_str() {
        "archive" => s.archive_dir,
        "downloads" => s.downloads_dir,
        "services" => state.config_dir.join("services"),
        "logs" => app.path().app_log_dir().map_err(|e| e.to_string())?,
        _ => return Err("unknown folder".into()),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) {
    webviews::open_external(&app, &url);
}

#[tauri::command]
pub async fn run_import(
    app: AppHandle,
    kind: ImportKind,
    path: String,
    account_id: String,
) -> Result<ImportReport, String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let emitter = app2.clone();
        importers::run(
            &state,
            kind,
            std::path::Path::new(&path),
            &account_id,
            &|p| {
                let _ = emitter.emit_to(webviews::SHELL, "import-progress", &p);
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn reveal_recovery_key(state: State<'_, AppState>) -> Result<String, String> {
    Ok(crate::crypto::recovery_string(&state.key()?))
}

/// Uses a recovery key (from another computer) to open the archive here.
#[tauri::command]
pub fn import_recovery_key(state: State<'_, AppState>, key: String) -> Result<(), String> {
    let k = crate::crypto::parse_recovery(&key)?;
    let dir = lock(&state.settings).archive_dir.clone();
    let opened = Archive::open(&dir, &k)?;
    crate::crypto::save_key(&state.config_dir, &k)?;
    *lock(&state.key) = Some(k);
    *lock(&state.archive) = Some(opened);
    *lock(&state.archive_error) = None;
    Ok(())
}

/// Adds an OpenAI-compatible provider (DeepSeek, MiniMax, OpenRouter, a local server…).
#[tauri::command]
pub fn add_provider(
    state: State<'_, AppState>,
    name: String,
    base_url: String,
) -> Result<ProviderConfig, String> {
    let url = url::Url::parse(base_url.trim()).map_err(|_| "Địa chỉ API không hợp lệ")?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(
            "Địa chỉ API phải bắt đầu bằng https:// (hoặc http:// cho máy chủ trong máy)".into(),
        );
    }
    let p = ProviderConfig {
        id: format!("custom-{}", crate::util::random_id(4)),
        kind: ProviderKind::Compatible,
        name: if name.trim().is_empty() {
            url.host_str().unwrap_or("API").to_string()
        } else {
            name.trim().to_string()
        },
        base_url: base_url.trim().trim_end_matches('/').to_string(),
        ..ProviderConfig::default()
    };
    let mut s = lock(&state.settings);
    s.ai.providers.push(p.clone());
    save(&state, &s)?;
    Ok(p)
}

#[tauri::command]
pub fn update_provider(state: State<'_, AppState>, provider: ProviderConfig) -> Result<(), String> {
    let mut s = lock(&state.settings);
    let slot =
        s.ai.providers
            .iter_mut()
            .find(|p| p.id == provider.id)
            .ok_or("Không tìm thấy provider")?;
    let kind = slot.kind;
    *slot = ProviderConfig { kind, ..provider };
    save(&state, &s)
}

#[tauri::command]
pub fn remove_provider(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut s = lock(&state.settings);
    let builtin = crate::settings::default_providers()
        .iter()
        .any(|p| p.id == id);
    if builtin {
        return Err("Provider có sẵn chỉ có thể tắt, không xóa được".into());
    }
    s.ai.providers.retain(|p| p.id != id);
    save(&state, &s)?;
    drop(s);
    lock(&state.secrets).set(&id, None)
}

/// Starts Mnemo hidden in the tray when Windows starts.
#[tauri::command]
pub fn set_autostart(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    apply_autostart(&app, &state, enabled)?;
    crate::refresh_tray_menu(&app);
    Ok(())
}

/// Turns "start with Windows" on or off (Windows' Run key) and remembers it.
pub fn apply_autostart(app: &AppHandle, state: &AppState, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    if enabled {
        launcher
            .enable()
            .map_err(|e| format!("Không bật được khởi động cùng Windows: {e}"))?;
    } else if launcher.is_enabled().unwrap_or(false) {
        launcher
            .disable()
            .map_err(|e| format!("Không tắt được khởi động cùng Windows: {e}"))?;
    }
    let mut s = lock(&state.settings);
    s.start_with_windows = enabled;
    save(&state, &s)
}

/// Turns an account off for now (its page closes and stops using memory; the sign-in stays
/// on disk) or back on (its page opens again, already signed in).
#[tauri::command]
pub async fn set_account_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut s = lock(&state.settings);
        let a = s
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("Không tìm thấy tài khoản")?;
        a.disabled = !enabled;
        save(&state, &s)?;
    }
    if enabled {
        if let Some(a) = state.settings().account(&id).cloned() {
            let wv = webviews::ensure_account(&app, &a)?;
            webviews::park(&app, &wv);
        }
    } else {
        webviews::close_account(&app, &id);
        lock(&state.unread).remove(&id);
        lock(&state.page_unread).remove(&id);
        lock(&state.notices).retain(|n| n.account != id);
    }
    crate::notify::update_taskbar(&app);
    let _ = app.emit_to(webviews::SHELL, "accounts", &id);
    Ok(())
}

/// Turns sharing on (every existing account joins a group, then Mnemo restarts so the
/// pages reopen in the shared browsers) or off (only new accounts get their own folder).
#[tauri::command]
pub fn share_processes(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut s = lock(&state.settings);
        s.share_processes = enabled;
        if enabled {
            crate::profiles::assign_all(&mut s);
        } else {
            crate::profiles::unshare_all(&mut s);
        }
        save(&state, &s)?;
    }
    // The folders move at the next start, before any page opens.
    app.restart();
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
