//! Mnemo – Every chat in sight, locked safe and tight.
//!
//! One window for WhatsApp, Messenger, Instagram, Telegram and Zalo, with a local,
//! encrypted archive of messages and media, and an AI writing/summary assistant.

pub mod ai;
pub mod archive;
pub mod capture;
pub mod commands;
pub mod crypto;
pub mod importers;
pub mod media;
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

fn build_state(app: &tauri::App) -> Result<AppState, Box<dyn std::error::Error>> {
    let config_dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    let documents = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())?;
    let settings = settings::Settings::load(&config_dir, &documents);
    let _ = settings.save(&config_dir);
    // Files decrypted for opening last time.
    let _ = std::fs::remove_dir_all(config_dir.join("temp-open"));
    let (key, archive_error) = match crypto::load_or_create_key(&config_dir) {
        Ok((k, _)) => (Some(k), None),
        Err(e) => (None, Some(e)),
    };
    let http = reqwest::Client::builder()
        .user_agent(concat!("Mnemo/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let state = AppState {
        secrets: Mutex::new(crypto::Secrets::load(&config_dir)),
        config_dir,
        settings: Mutex::new(settings),
        archive: Mutex::new(None),
        archive_error: Mutex::new(archive_error),
        key: Mutex::new(key),
        unread: Mutex::new(HashMap::new()),
        stats: Mutex::new(HashMap::new()),
        context: Mutex::new(HashMap::new()),
        visible: Mutex::new(None),
        panel_open: Mutex::new(false),
        assist: Mutex::new(Default::default()),
        http,
    };
    if key.is_some() {
        state.open_archive();
    }
    Ok(state)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_window(webviews::MAIN) {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
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

            let window = WindowBuilder::new(app, webviews::MAIN)
                .title("Mnemo")
                .inner_size(1360.0, 860.0)
                .min_inner_size(900.0, 560.0)
                .center()
                .build()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            let size = window.inner_size()?.to_logical::<f64>(scale);
            window.add_child(
                WebviewBuilder::new(webviews::SHELL, WebviewUrl::App("index.html".into())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(size.width, size.height),
            )?;

            let handle = app.handle().clone();
            window.on_window_event(move |e| {
                if let WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } = e {
                    webviews::layout(&handle);
                }
            });

            // Open the accounts' pages in the background so messages keep arriving.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(800));
                let accounts = handle.state::<AppState>().settings().accounts;
                for (i, a) in accounts.iter().enumerate() {
                    let h = handle.clone();
                    let acc = a.clone();
                    let first = i == 0;
                    let _ = handle.run_on_main_thread(move || {
                        if let Ok(wv) = webviews::ensure_account(&h, &acc) {
                            if !first
                                || lock(&h.state::<AppState>().visible).as_deref()
                                    != Some(acc.id.as_str())
                            {
                                let _ = wv.hide();
                            }
                        }
                    });
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
            capture::mnemo_ingest,
            capture::mnemo_media_blob,
            capture::mnemo_media_url,
            ai::mnemo_assist_open,
            ai::mnemo_assist_check,
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
            commands::remove_account,
            commands::show_account,
            commands::show_page,
            commands::reload_account,
            commands::sleep_account,
            commands::open_devtools,
            commands::set_panel,
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
