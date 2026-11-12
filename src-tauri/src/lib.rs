//! Mnemo – Every chat in sight, locked safe and tight.
//!
//! One window for WhatsApp, Messenger, Instagram, Telegram and Zalo, with a local,
//! encrypted archive of messages and media, and an AI writing/summary assistant.

pub mod ai;
pub mod archive;
pub mod capture;
pub mod commands;
pub mod crypto;
pub mod icons;
pub mod importers;
pub mod media;
pub mod notify;
pub mod profiles;
pub mod rules;
pub mod services;
pub mod settings;
pub mod state;
pub mod util;
pub mod webviews;

use state::{AppState, lock};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowBuilder, WindowEvent,
};

/// Deletes automatically saved media past the retention period.
fn apply_retention(state: &AppState) {
    let s = state.settings();
    let mut days = s.rules.retention_days;
    // The shortest per-account retention wins for the shared files.
    for a in &s.accounts {
        if let Some(r) = &a.rules {
            if r.retention_days > 0 && (days == 0 || r.retention_days < days) {
                days = r.retention_days;
            }
        }
    }
    if days == 0 {
        return;
    }
    let cutoff = util::now_ms() - days as i64 * 86_400_000;
    match state.with_archive(|a| a.expire_media(cutoff)) {
        Ok(files) => {
            for f in files {
                let _ = std::fs::remove_file(f);
            }
        }
        Err(e) => log::warn!("retention: {e}"),
    }
}

fn load_avatars(config_dir: &std::path::Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for e in std::fs::read_dir(config_dir.join("avatars"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let p = e.path();
        if p.extension().map(|x| x == "txt").unwrap_or(false) {
            if let (Some(id), Ok(data)) = (p.file_stem(), std::fs::read_to_string(&p)) {
                if data.starts_with("data:image/") {
                    out.insert(id.to_string_lossy().into_owned(), data);
                }
            }
        }
    }
    out
}

fn build_state(app: &tauri::App) -> Result<AppState, Box<dyn std::error::Error>> {
    let config_dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    let documents = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())?;
    let mut settings = settings::Settings::load(&config_dir, &documents);
    // New installs share browser processes from the start.
    if settings.share_processes {
        profiles::assign_all(&mut settings);
    }
    profiles::migrate(&config_dir, &mut settings);
    let _ = settings.save(&config_dir);
    // The interface's own browser data of earlier versions (it now lives with the accounts').
    if let Ok(local) = app.path().app_local_data_dir() {
        let _ = std::fs::remove_dir_all(local.join("EBWebView"));
    }
    // Files decrypted for opening last time.
    let _ = std::fs::remove_dir_all(config_dir.join("temp-open"));
    let (key, archive_error) = match crypto::load_or_create_key(&config_dir) {
        Ok((k, _)) => (Some(k), None),
        Err(e) => (None, Some(e)),
    };
    let http = reqwest::Client::builder()
        .user_agent(concat!("Mnemo/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let config_dir_for_avatars = config_dir.clone();
    let sidebar_pos = settings.sidebar_pos.clone();
    let state = AppState {
        secrets: Mutex::new(crypto::Secrets::load(&config_dir)),
        config_dir,
        settings: Mutex::new(settings),
        archive: Mutex::new(None),
        archive_error: Mutex::new(archive_error),
        key: Mutex::new(key),
        unread: Mutex::new(HashMap::new()),
        pending: Mutex::new(HashMap::new()),
        stats: Mutex::new(HashMap::new()),
        context: Mutex::new(HashMap::new()),
        visible: Mutex::new(None),
        panel_open: Mutex::new(false),
        assist: Mutex::new(Default::default()),
        sidebar_w: Mutex::new(webviews::SIDEBAR_W),
        sidebar_pos: Mutex::new(sidebar_pos),
        avatars: Mutex::new(load_avatars(&config_dir_for_avatars)),
        notices: Mutex::new(vec![]),
        senders: Mutex::new(HashMap::new()),
        page_unread: Mutex::new(Default::default()),
        crop_top: Mutex::new(HashMap::new()),
        writing: Mutex::new(HashMap::new()),
        icons: Mutex::new(icons::load_cached(&config_dir_for_avatars)),
        http,
    };
    if key.is_some() {
        state.open_archive();
    }
    Ok(state)
}

const HIDDEN_ARG: &str = "--hidden";

/// Brings the main window back (from the tray, minimized or behind other windows).
pub fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_window(webviews::MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    webviews::layout(app);
}

/// The tray menu in the interface language.
pub fn tray_menu(app: &tauri::AppHandle, en: bool) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
    let s = app.state::<AppState>().settings();
    let t = |vi: &'static str, en_text: &'static str| if en { en_text } else { vi };
    let open = MenuItemBuilder::with_id("open", t("Mở Mnemo", "Open Mnemo")).build(app)?;
    let autostart = CheckMenuItemBuilder::with_id(
        "autostart",
        t("Khởi động cùng Windows", "Start with Windows (in the tray)"),
    )
    .checked(s.start_with_windows)
    .build(app)?;
    let tray = CheckMenuItemBuilder::with_id(
        "close_to_tray",
        t(
            "Bấm × thì thu về khay",
            "Closing the window minimizes to the tray",
        ),
    )
    .checked(s.close_to_tray)
    .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", t("Thoát Mnemo", "Quit Mnemo")).build(app)?;
    MenuBuilder::new(app)
        .item(&open)
        .separator()
        .item(&autostart)
        .item(&tray)
        .separator()
        .item(&quit)
        .build()
}

/// Rebuilds the tray menu (language or a checkbox changed).
pub fn refresh_tray_menu(app: &tauri::AppHandle) {
    let en = app.state::<AppState>().settings().language == "en";
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("mnemo"), tray_menu(app, en)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let en = app.state::<AppState>().settings().language == "en";
    let menu = tray_menu(app.handle(), en)?;
    let mut tray = TrayIconBuilder::with_id("mnemo")
        .tooltip("Mnemo – Every chat in sight, locked safe and tight.")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            "autostart" => {
                let state = app.state::<AppState>();
                let on = !state.settings().start_with_windows;
                if let Err(e) = commands::apply_autostart(app, &state, on) {
                    log::warn!("{e}");
                }
                refresh_tray_menu(app);
                let _ = tauri::Emitter::emit_to(app, webviews::SHELL, "settings", ());
            }
            "close_to_tray" => {
                let state = app.state::<AppState>();
                {
                    let mut s = lock(&state.settings);
                    s.close_to_tray = !s.close_to_tray;
                    let _ = commands::save(&state, &s);
                }
                refresh_tray_menu(app);
                let _ = tauri::Emitter::emit_to(app, webviews::SHELL, "settings", ());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_ARG]),
        ))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = build_state(app)?;
            app.manage(state);

            // Started with Windows: stay in the tray, pages keep running and notifying.
            let hidden = std::env::args().any(|a| a == HIDDEN_ARG);
            let start_minimized = hidden && app.state::<AppState>().settings().start_minimized;
            let window = WindowBuilder::new(app, webviews::MAIN)
                .title("Mnemo")
                .inner_size(1360.0, 860.0)
                .min_inner_size(900.0, 560.0)
                .center()
                .visible(!hidden)
                .build()?;
            build_tray(app)?;
            if start_minimized {
                // On the taskbar (with the unread count), not opened.
                let _ = window.minimize();
                let _ = window.show();
                let _ = window.minimize();
            }
            let scale = window.scale_factor().unwrap_or(1.0);
            let size = window.inner_size()?.to_logical::<f64>(scale);
            window.add_child(
                WebviewBuilder::new(webviews::SHELL, WebviewUrl::App("index.html".into()))
                    .additional_browser_args(webviews::BROWSER_ARGS)
                    .data_directory(webviews::own_dir(app.handle())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(size.width, size.height),
            )?;

            let handle = app.handle().clone();
            window.on_window_event(move |e| match e {
                WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::Focused(true) => {
                    if matches!(e, WindowEvent::Focused(true)) {
                        let shown = lock(&handle.state::<AppState>().visible).clone();
                        if let Some(id) = shown {
                            webviews::seen(&handle, &id);
                        }
                    }
                    webviews::layout(&handle)
                }
                WindowEvent::CloseRequested { api, .. } => {
                    if handle.state::<AppState>().settings().close_to_tray {
                        api.prevent_close();
                        if let Some(w) = handle.get_window(webviews::MAIN) {
                            let _ = w.hide();
                        }
                        webviews::hide_assist(&handle);
                        webviews::layout(&handle);
                    } else {
                        handle.exit(0);
                    }
                }
                _ => {}
            });

            tauri::async_runtime::spawn(icons::refresh(app.handle().clone()));

            // Open the accounts' pages in the background so messages keep arriving.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(800));
                notify::warm_up(&handle);
                let accounts = handle.state::<AppState>().settings().accounts;
                for (i, a) in accounts.iter().filter(|a| !a.disabled).enumerate() {
                    // Created from this worker thread: creating a webview from the main
                    // thread's own callbacks waits on the main thread and freezes the window.
                    let _ = i;
                    if let Ok(wv) = webviews::ensure_account(&handle, a) {
                        if lock(&handle.state::<AppState>().visible).as_deref()
                            != Some(a.id.as_str())
                        {
                            webviews::park(&handle, &wv);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
            });

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    apply_retention(&handle.state::<AppState>());
                    std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // The writing popup closes when it loses focus.
            if window.label() == webviews::ASSIST {
                if let WindowEvent::Focused(false) = event {
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // pages of the apps
            capture::mnemo_config,
            capture::mnemo_log,
            capture::mnemo_context,
            capture::mnemo_senders,
            capture::mnemo_crop,
            capture::mnemo_ingest,
            capture::mnemo_media_blob,
            capture::mnemo_media_url,
            ai::mnemo_assist_open,
            ai::mnemo_assist_check,
            ai::mnemo_assist_learn,
            ai::mnemo_writing,
            ai::writing_state,
            ai::writing_action,
            ai::writing_apply,
            ai::set_auto_check,
            ai::assist_learn,
            ai::assist_save_goals,
            // writing popup
            ai::assist_state,
            ai::assist_run,
            ai::assist_apply,
            ai::assist_close,
            ai::assist_save_choice,
            // shell
            commands::get_state,
            commands::save_settings,
            commands::add_account,
            commands::update_account,
            commands::move_account,
            commands::set_account_enabled,
            webviews::open_picker,
            webviews::picker_state,
            webviews::picker_close,
            commands::share_processes,
            commands::restart_app,
            commands::reorder_accounts,
            commands::remove_account,
            commands::show_account,
            commands::show_page,
            commands::reload_account,
            commands::sleep_account,
            commands::open_devtools,
            commands::set_layout,
            commands::set_autostart,
            commands::quit_app,
            commands::set_account_avatar,
            capture::mnemo_profile,
            notify::mnemo_notify,
            notify::notices_state,
            notify::notice_dismiss,
            notify::notice_open,
            commands::pick_folder,
            commands::pick_file,
            commands::set_archive_dir,
            commands::set_downloads_dir,
            commands::list_chats,
            commands::chat_messages,
            commands::search_messages,
            commands::mark_seen,
            commands::live_chat,
            commands::list_media,
            commands::media_preview,
            commands::open_media,
            commands::reveal_media,
            commands::open_folder,
            commands::open_url,
            commands::run_import,
            commands::reveal_recovery_key,
            commands::import_recovery_key,
            commands::add_provider,
            commands::update_provider,
            commands::remove_provider,
            ai::ai_providers,
            ai::ai_set_key,
            ai::ai_list_models,
            ai::ai_test,
            ai::ai_import_skill,
            ai::ai_summarize,
            ai::ai_suggest_replies,
            ai::ai_ask,
            ai::ai_insert_reply,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mnemo");
}
