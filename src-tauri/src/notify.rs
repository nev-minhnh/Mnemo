//! Mnemo's own notifications for new messages, and the unread count on the taskbar.
//!
//! The apps' pages don't show notifications themselves: the injected script hands every
//! notification they try to show to Mnemo (`mnemo_notify`). Mnemo shows it in a small
//! always-on-top window in the corner of the screen ([App] / From / To / time / text), and a
//! click opens Mnemo on that account and replays the app's own click, which takes it to
//! the conversation and message.

use crate::state::{AppState, lock};
use crate::webviews;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State, Webview, WebviewUrl};

pub const NOTIFY: &str = "notify";
const CARD_H: f64 = 132.0;
const WIDTH: f64 = 392.0;
const MAX_SHOWN: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notice {
    pub id: String,
    pub account: String,
    pub service: String,
    pub app: String,
    pub from: String,
    pub to: String,
    pub text: String,
    pub ts: i64,
    pub avatar: Option<String>,
    /// The page's own notification id, to replay its click.
    pub page_id: Option<String>,
    /// Extra routing data some apps give (Telegram: chatId, messageId).
    pub extra: Value,
    #[serde(default)]
    pub generic: bool,
}

fn notify_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(NOTIFY) {
        return Ok(w);
    }
    tauri::WebviewWindowBuilder::new(app, NOTIFY, WebviewUrl::App("notify.html".into()))
        .title("Mnemo")
        .inner_size(WIDTH, CARD_H)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focusable(false)
        .focused(false)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())
}

/// Sizes the window to the cards and puts it in the bottom-right corner of the work area.
fn place(app: &AppHandle, count: usize) {
    let Ok(w) = notify_window(app) else { return };
    if count == 0 {
        let _ = w.hide();
        return;
    }
    let shown = count.min(MAX_SHOWN) as f64;
    let height = shown * CARD_H + 8.0;
    let monitor = app
        .get_window(webviews::MAIN)
        .and_then(|m| m.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let scale = m.scale_factor();
        let area = m.work_area();
        let x = area.position.x as f64 + area.size.width as f64 - (WIDTH + 12.0) * scale;
        let y = area.position.y as f64 + area.size.height as f64 - (height + 12.0) * scale;
        let _ = w.set_size(tauri::LogicalSize::new(WIDTH, height));
        let _ = w.set_position(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ));
    }
    let _ = w.show();
}

fn publish(app: &AppHandle) {
    let state = app.state::<AppState>();
    let list = lock(&state.notices).clone();
    place(app, list.len());
    let st = state.settings();
    let _ = app.emit_to(
        NOTIFY,
        "notices",
        json!({
            "notices": list,
            "duration": st.notify.duration_secs.clamp(3, 120),
            "language": st.language,
            "theme": st.theme,
            "icons": *lock(&state.icons),
        }),
    );
}

pub fn push(app: &AppHandle, notice: Notice) {
    let state = app.state::<AppState>();
    {
        let mut n = lock(&state.notices);
        n.retain(|x| {
            !(x.account == notice.account && x.page_id.is_some() && x.page_id == notice.page_id)
        });
        n.insert(0, notice);
        n.truncate(20);
    }
    publish(app);
}

/// Display name of an account for the "To:" line.
pub fn account_name(state: &AppState, account_id: &str) -> String {
    state
        .settings()
        .account(account_id)
        .map(|a| a.label.clone())
        .unwrap_or_default()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageNotice {
    id: Option<String>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    extra: Value,
}

/// Called by an app's page instead of showing its own notification.
#[tauri::command]
pub async fn mnemo_notify(
    app: AppHandle,
    webview: Webview,
    state: State<'_, AppState>,
    notice: PageNotice,
) -> Result<(), String> {
    let s = state.settings();
    let account = crate::capture::account_for(&webview, &s)?;
    if !s.notify.enabled || !account.notify {
        return Ok(());
    }
    // The person is looking at this conversation right now.
    let main_focused = app
        .get_window(webviews::MAIN)
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    let visible = lock(&state.visible).as_deref() == Some(account.id.as_str());
    let same_chat = lock(&state.context)
        .get(&account.id)
        .and_then(|c| c.title.clone())
        .map(|t| !t.is_empty() && t.trim() == notice.title.trim())
        .unwrap_or(false);
    if main_focused && visible && same_chat {
        return Ok(());
    }
    let text = if s.notify.show_content {
        crate::util::truncate_chars(notice.body.trim(), 600)
    } else {
        "Tin nhắn mới".to_string()
    };
    let extra = if notice.extra.to_string().len() <= 2000 {
        notice.extra
    } else {
        Value::Null
    };
    push(
        &app,
        Notice {
            id: crate::util::random_id(8),
            account: account.id.clone(),
            service: account.service.as_str().into(),
            app: account.service.display().into(),
            from: crate::util::truncate_chars(notice.title.trim(), 200),
            to: account.label.clone(),
            text,
            ts: crate::util::now_ms(),
            avatar: lock(&state.avatars).get(&account.id).cloned(),
            page_id: notice.id.map(|i| crate::util::truncate_chars(&i, 100)),
            extra,
            generic: false,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn notices_state(state: State<'_, AppState>) -> Value {
    let st = state.settings();
    json!({
        "notices": *lock(&state.notices),
        "duration": st.notify.duration_secs.clamp(3, 120),
        "language": st.language,
        "theme": st.theme,
        "icons": *lock(&state.icons),
    })
}

#[tauri::command]
pub fn notice_dismiss(app: AppHandle, state: State<'_, AppState>, id: Option<String>) {
    {
        let mut n = lock(&state.notices);
        match id {
            Some(id) => n.retain(|x| x.id != id),
            None => n.clear(),
        }
    }
    publish(&app);
}

/// Opens Mnemo on the notice's account and replays the app's own click on it.
#[tauri::command]
pub async fn notice_open(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let notice = {
        let mut n = lock(&state.notices);
        let pos = n
            .iter()
            .position(|x| x.id == id)
            .ok_or("Thông báo đã hết hạn")?;
        n.remove(pos)
    };
    publish(&app);
    if let Some(w) = app.get_window(webviews::MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    webviews::show_account(&app, &notice.account)?;
    let _ = app.emit_to(
        webviews::SHELL,
        "visible",
        json!({ "account": notice.account }),
    );
    if let Some(wv) = app.get_webview(&webviews::label_for(&notice.account)) {
        let payload =
            json!({ "pageId": notice.page_id, "title": notice.from, "extra": notice.extra });
        let _ = wv.eval(format!(
            "window.__MNEMO_NOTICE_CLICK__ && window.__MNEMO_NOTICE_CLICK__({payload});"
        ));
    }
    Ok(())
}

/// When an app's unread count goes up but it didn't hand over a notification
/// (some apps only notify in certain states), show a short generic one.
pub fn unread_rose(app: &AppHandle, account_id: &str, before: u32, now: u32) {
    let app = app.clone();
    let account_id = account_id.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));
        let state = app.state::<AppState>();
        let s = state.settings();
        if !s.notify.enabled {
            return;
        }
        let Some(account) = s.account(&account_id).cloned() else {
            return;
        };
        if !account.notify {
            return;
        }
        // The count dropped again (read meanwhile) or the page sent a real notification.
        if lock(&state.unread).get(&account_id).copied().unwrap_or(0) <= before {
            return;
        }
        let recent = lock(&state.notices)
            .iter()
            .any(|n| n.account == account_id && !n.generic && crate::util::now_ms() - n.ts < 8000);
        let main_focused = app
            .get_window(webviews::MAIN)
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        let visible = lock(&state.visible).as_deref() == Some(account_id.as_str());
        if recent || (main_focused && visible) {
            return;
        }
        let new = now.saturating_sub(before).max(1);
        push(
            &app,
            Notice {
                id: crate::util::random_id(8),
                account: account_id.clone(),
                service: account.service.as_str().into(),
                app: account.service.display().into(),
                from: "—".into(),
                to: account.label.clone(),
                text: if state.settings().language == "en" {
                    format!("{new} new messages ({now} unread)")
                } else {
                    format!("{new} tin nhắn mới ({now} chưa đọc)")
                },
                ts: crate::util::now_ms(),
                avatar: lock(&state.avatars).get(&account_id).cloned(),
                page_id: None,
                extra: Value::Null,
                generic: true,
            },
        );
    });
}

// ------------------------------------------------------------ taskbar badge

/// 3×5 pixel digits (and '+'), one row per u8, high bits first.
const GLYPHS: [[u8; 5]; 11] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
    [0b000, 0b010, 0b111, 0b010, 0b000],
];

/// A 32×32 red badge with the count in white (RGBA).
pub fn badge_rgba(count: u32) -> Vec<u8> {
    const S: usize = 32;
    let mut px = vec![0u8; S * S * 4];
    let text: Vec<usize> = if count > 99 {
        vec![9, 9, 10]
    } else {
        count
            .to_string()
            .bytes()
            .map(|b| (b - b'0') as usize)
            .collect()
    };
    // Red disc with a soft edge.
    for y in 0..S {
        for x in 0..S {
            let dx = x as f64 + 0.5 - 16.0;
            let dy = y as f64 + 0.5 - 16.0;
            let d = (dx * dx + dy * dy).sqrt();
            let a = (15.5 - d).clamp(0.0, 1.0);
            let i = (y * S + x) * 4;
            px[i..i + 4].copy_from_slice(&[0xE0, 0x2B, 0x3B, (a * 255.0) as u8]);
        }
    }
    let scale = if text.len() >= 3 { 2 } else { 4 };
    let gap = if text.len() >= 3 { 1 } else { 2 };
    let w = text.len() * 3 * scale + (text.len() - 1) * gap;
    let h = 5 * scale;
    let ox = (S - w) / 2;
    let oy = (S - h) / 2;
    for (n, g) in text.iter().enumerate() {
        let gx = ox + n * (3 * scale + gap);
        for (row, bits) in GLYPHS[*g].iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for yy in 0..scale {
                        for xx in 0..scale {
                            let x = gx + col * scale + xx;
                            let y = oy + row * scale + yy;
                            let i = (y * S + x) * 4;
                            px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
    px
}

/// Total unread over every account, on the taskbar icon and in the window title.
pub fn update_taskbar(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings();
    let ids: Vec<String> = s.accounts.iter().map(|a| a.id.clone()).collect();
    let total: u32 = {
        let u = lock(&state.unread);
        ids.iter().filter_map(|id| u.get(id)).sum()
    };
    if let Some(tray) = app.tray_by_id("mnemo") {
        let tip = if total > 0 {
            if s.language == "en" {
                format!("Mnemo – {total} unread")
            } else {
                format!("Mnemo – {total} tin chưa đọc")
            }
        } else {
            "Mnemo".to_string()
        };
        let _ = tray.set_tooltip(Some(tip));
    }
    let Some(w) = app.get_window(webviews::MAIN) else {
        return;
    };
    let _ = w.set_title(&if total > 0 {
        format!("Mnemo ({total})")
    } else {
        "Mnemo".to_string()
    });
    #[cfg(windows)]
    {
        let icon = if s.notify.taskbar_badge && total > 0 {
            Some(tauri::image::Image::new_owned(badge_rgba(total), 32, 32))
        } else {
            None
        };
        let _ = w.set_overlay_icon(icon);
    }
    #[cfg(not(windows))]
    {
        let _ = w.set_badge_count(if s.notify.taskbar_badge && total > 0 {
            Some(total as i64)
        } else {
            None
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn badge_draws_digits() {
        for n in [1, 7, 42, 99, 100, 5000] {
            let px = super::badge_rgba(n);
            assert_eq!(px.len(), 32 * 32 * 4);
            let white = px.chunks(4).filter(|p| p == &[255, 255, 255, 255]).count();
            assert!(white > 10, "{n}: digits drawn");
        }
    }
}
