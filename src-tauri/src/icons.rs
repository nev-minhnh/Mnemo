//! The apps' own icons, loaded at run time from each service's website (the same favicon
//! a browser tab shows) and cached in `<config>\icons`. Nothing is bundled with Mnemo.

use crate::settings::ServiceId;
use crate::state::{AppState, lock};
use tauri::{AppHandle, Emitter, Manager};

fn candidates(id: ServiceId) -> Vec<String> {
    let s2 = |d: &str| format!("https://www.google.com/s2/favicons?domain={d}&sz=128");
    match id {
        ServiceId::Whatsapp => vec![
            s2("web.whatsapp.com"),
            "https://web.whatsapp.com/favicon.ico".into(),
        ],
        ServiceId::Messenger => vec![
            s2("messenger.com"),
            "https://www.messenger.com/favicon.ico".into(),
        ],
        ServiceId::Instagram => vec![
            s2("instagram.com"),
            "https://www.instagram.com/favicon.ico".into(),
        ],
        ServiceId::Telegram => vec![
            s2("telegram.org"),
            "https://web.telegram.org/favicon.ico".into(),
        ],
        ServiceId::Zalo => vec![
            s2("chat.zalo.me"),
            "https://chat.zalo.me/favicon.ico".into(),
        ],
        ServiceId::Discord => vec![s2("discord.com"), "https://discord.com/favicon.ico".into()],
        ServiceId::Wechat => vec![
            s2("wechat.com"),
            "https://web.wechat.com/favicon.ico".into(),
        ],
        ServiceId::Threads => vec![
            s2("threads.com"),
            "https://www.threads.com/favicon.ico".into(),
        ],
        ServiceId::Slack => vec![s2("slack.com"), "https://slack.com/favicon.ico".into()],
        ServiceId::Linkedin => vec![
            s2("linkedin.com"),
            "https://www.linkedin.com/favicon.ico".into(),
        ],
    }
}

fn path(state: &AppState, id: ServiceId) -> std::path::PathBuf {
    state
        .config_dir
        .join("icons")
        .join(format!("{}.txt", id.as_str()))
}

pub fn load_cached(config_dir: &std::path::Path) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for id in ServiceId::ALL {
        if let Ok(d) = std::fs::read_to_string(
            config_dir
                .join("icons")
                .join(format!("{}.txt", id.as_str())),
        ) {
            if d.starts_with("data:image/") {
                out.insert(id.as_str().to_string(), d);
            }
        }
    }
    out
}

/// Fetches icons that aren't cached yet (or are older than 30 days).
pub async fn refresh(app: AppHandle) {
    let state = app.state::<AppState>();
    let mut changed = false;
    for id in ServiceId::ALL {
        let p = path(&state, id);
        let fresh = std::fs::metadata(&p)
            .and_then(|m| m.modified())
            .map(|t| {
                t.elapsed()
                    .map(|e| e.as_secs() < 30 * 86_400)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if fresh && lock(&state.icons).contains_key(id.as_str()) {
            continue;
        }
        for url in candidates(id) {
            let Ok(resp) = state
                .http
                .get(&url)
                .timeout(std::time::Duration::from_secs(20))
                .send()
                .await
            else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            let Ok(bytes) = resp.bytes().await else {
                continue;
            };
            if let Some(data) = crate::capture::image_data_url(&bytes, "") {
                let _ = crate::util::write_atomic(&p, data.as_bytes());
                lock(&state.icons).insert(id.as_str().to_string(), data);
                changed = true;
                break;
            }
        }
    }
    if changed {
        let _ = app.emit_to(crate::webviews::SHELL, "icons", &*lock(&state.icons));
    }
}
