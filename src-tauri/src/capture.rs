//! Commands the apps' pages call (through the injected script) to save what they show.
//!
//! These are the only commands a remote page may call (see capabilities/services.json).
//! The account is always taken from the calling webview's label, never from the payload,
//! and every field is size-checked.

use crate::archive::NewMessage;
use crate::media;
use crate::rules::{self, Candidate};
use crate::settings::{Account, Settings};
use crate::state::{AppState, LiveContext, lock};
use crate::webviews::{self, SHELL};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, State, Webview};

const MAX_BATCH: usize = 500;
const MAX_TEXT: usize = 50_000;
const MAX_KEY: usize = 400;

pub fn account_for(webview: &Webview, settings: &Settings) -> Result<Account, String> {
    let id = webviews::account_of_label(webview.label()).ok_or("not an account page")?;
    settings
        .account(id)
        .cloned()
        .ok_or_else(|| "unknown account".into())
}

#[tauri::command]
pub fn mnemo_config(webview: Webview, state: State<'_, AppState>) -> Result<Value, String> {
    let s = state.settings();
    let account = account_for(&webview, &s)?;
    let rules = s.rules_for(&account.id);
    Ok(json!({
        "capture": s.capture_enabled && account.capture,
        "rules": {
            "enabled": rules.enabled && s.capture_enabled && account.capture,
            "images": rules.images, "videos": rules.videos, "audio": rules.audio,
            "documents": rules.documents, "other": rules.other,
            "maxBytes": rules::max_bytes(&rules),
        },
        "autoCheck": s.ai.auto_check,
        "assist": true,
        "hotkey": "Ctrl+Shift+Space",
    }))
}

#[tauri::command]
pub fn mnemo_log(webview: Webview, message: String) {
    log::info!(
        "[{}] {}",
        webview.label(),
        crate::util::truncate_chars(&message, 500)
    );
}

#[tauri::command]
pub fn mnemo_context(
    app: AppHandle,
    webview: Webview,
    state: State<'_, AppState>,
    chat_key: Option<String>,
    title: Option<String>,
    is_group: bool,
) -> Result<(), String> {
    let s = state.settings();
    let account = account_for(&webview, &s)?;
    let ctx = LiveContext {
        chat_key: chat_key.map(|k| crate::util::truncate_chars(&k, MAX_KEY)),
        title: title.map(|t| crate::util::truncate_chars(&t, 300)),
        is_group,
    };
    lock(&state.context).insert(account.id.clone(), ctx.clone());
    let _ = app.emit_to(
        SHELL,
        "context",
        json!({ "account": account.id, "context": ctx }),
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMessage {
    key: String,
    sender: Option<String>,
    outgoing: Option<bool>,
    ts: Option<f64>,
    time_text: Option<String>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    recalled: bool,
    prev_key: Option<String>,
    next_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBatch {
    chat_key: String,
    title: Option<String>,
    #[serde(default)]
    is_group: bool,
    read_up_to_key: Option<String>,
    #[serde(default)]
    messages: Vec<LiveMessage>,
}

#[tauri::command]
pub async fn mnemo_ingest(
    app: AppHandle,
    webview: Webview,
    state: State<'_, AppState>,
    batch: CaptureBatch,
) -> Result<Value, String> {
    let s = state.settings();
    let account = account_for(&webview, &s)?;
    if !(s.capture_enabled && account.capture) {
        return Ok(json!({ "saved": 0, "skipped": "capture off" }));
    }
    if batch.chat_key.trim().is_empty()
        || batch.chat_key.len() > MAX_KEY
        || batch.messages.len() > MAX_BATCH
    {
        return Err("invalid batch".into());
    }
    let now = crate::util::now_ms();
    let msgs: Vec<NewMessage> = batch
        .messages
        .into_iter()
        .filter(|m| !m.key.is_empty() && m.key.len() <= MAX_KEY)
        .map(|m| NewMessage {
            key: m.key,
            sender: m.sender.map(|x| crate::util::truncate_chars(&x, 200)),
            outgoing: m.outgoing,
            // Times far in the future are a parsing mistake; keep them inexact instead.
            ts: m
                .ts
                .map(|t| t as i64)
                .filter(|t| *t > 946_684_800_000 && *t < now + 86_400_000),
            time_text: m.time_text.map(|x| crate::util::truncate_chars(&x, 80)),
            text: crate::util::truncate_chars(&m.text, MAX_TEXT),
            kind: crate::util::truncate_chars(&m.kind, 20),
            reply_to: None,
            source: "live".into(),
            recalled: m.recalled,
            prev_key: m.prev_key.filter(|k| k.len() <= MAX_KEY),
            next_key: m.next_key.filter(|k| k.len() <= MAX_KEY),
        })
        .collect();
    let service = account.service.as_str();
    let keep = s.keep_recalled;
    let (inserted, files) = state.with_archive(|a| {
        a.transaction(|a| {
            let chat_id = a.upsert_chat(
                service,
                &account.id,
                &batch.chat_key,
                batch.title.as_deref(),
                Some(batch.is_group),
            )?;
            let (results, files, _) = a.write_batch(chat_id, &msgs, now, keep)?;
            if let Some(k) = &batch.read_up_to_key {
                a.set_read_up_to_key(chat_id, k)?;
            }
            let inserted = results
                .iter()
                .filter(|(_, o)| *o == crate::archive::Outcome::Inserted)
                .count();
            Ok((inserted, files))
        })
    })?;
    for f in files {
        let _ = std::fs::remove_file(f);
    }
    if inserted > 0 {
        state.bump_stats(&account.id, inserted as u64, 0, None);
        let _ = app.emit_to(
            SHELL,
            "captured",
            json!({ "account": account.id, "count": inserted }),
        );
    }
    Ok(json!({ "saved": inserted }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMeta {
    chat_key: String,
    msg_key: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    mime: String,
}

/// Saves one piece of media shown in a chat, if the rules allow it.
fn save_live_media(
    state: &AppState,
    account: &Account,
    meta: &MediaMeta,
    data: &[u8],
    mime: &str,
) -> Result<bool, String> {
    let s = state.settings();
    let rules = s.rules_for(&account.id);
    let service = account.service.as_str();
    let (chat_id, title, is_group, message_id) = state.with_archive(|a| {
        let chat_id = a
            .find_chat(service, &account.id, &meta.chat_key)?
            .ok_or("chat not archived yet")?;
        let chat = a.chat(chat_id)?.ok_or("chat not archived yet")?;
        let mid = a.message_id(chat_id, &meta.msg_key)?;
        Ok((chat_id, chat.title, chat.is_group, mid))
    })?;
    let mut name = crate::util::safe_component(meta.name.trim(), 120);
    let mut ext = crate::util::ext_of(&name);
    if ext.is_empty() || name == "_" {
        name = media::fallback_name(
            if meta.kind.is_empty() {
                "media"
            } else {
                &meta.kind
            },
            mime,
            crate::util::now_ms(),
        );
        ext = crate::util::ext_of(&name);
    }
    let cand = Candidate {
        ext: &ext,
        mime,
        size: data.len() as u64,
        is_group,
        chat_key: &meta.chat_key,
        chat_title: &title,
    };
    if rules::decide(&rules, &cand).is_err() {
        return Ok(false);
    }
    let sha = crate::util::sha256_hex(data);
    if let Some(mid) = message_id {
        if state.with_archive(|a| a.media_linked(mid, &sha))? {
            return Ok(false);
        }
    }
    let key = state.key()?;
    let encrypted = s.encrypt_media;
    let existing = state.with_archive(|a| a.file_by_hash(&sha, encrypted))?;
    let (path, size) = match existing {
        Some(p) => (p, data.len() as u64),
        None => {
            let dir = media::chat_dir(
                &s.downloads_dir,
                account.service,
                &account.label,
                &title,
                &meta.chat_key,
                crate::util::now_ms(),
            );
            let w = media::write_new(&dir, &name, data, encrypted.then_some(&key))?;
            (w.path, w.size)
        }
    };
    let kind = rules::categorize(&ext, mime).as_str();
    state.with_archive(|a| {
        a.add_media(&crate::archive::NewMedia {
            message_id,
            chat_id: Some(chat_id),
            kind,
            file_name: &name,
            mime,
            size: size as i64,
            sha256: &sha,
            path: &path,
            encrypted,
            origin: "live",
        })
    })?;
    state.bump_stats(&account.id, 0, 1, None);
    Ok(true)
}

#[tauri::command]
pub async fn mnemo_media_blob(
    webview: Webview,
    state: State<'_, AppState>,
    meta: MediaMeta,
    data: String,
) -> Result<bool, String> {
    let s = state.settings();
    let account = account_for(&webview, &s)?;
    if !(s.capture_enabled && account.capture) {
        return Ok(false);
    }
    let cap = rules::max_bytes(&s.rules_for(&account.id));
    if (data.len() as u64) / 4 * 3 > cap + 3 {
        return Ok(false);
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|_| "bad media data")?;
    let mime = crate::util::truncate_chars(&meta.mime, 100);
    save_live_media(&state, &account, &meta, &bytes, &mime)
}

#[tauri::command]
pub async fn mnemo_media_url(
    webview: Webview,
    state: State<'_, AppState>,
    meta: MediaMeta,
    url: String,
) -> Result<bool, String> {
    use futures_util::StreamExt;
    let s = state.settings();
    let account = account_for(&webview, &s)?;
    if !(s.capture_enabled && account.capture) {
        return Ok(false);
    }
    let parsed = url::Url::parse(&url).map_err(|_| "bad url")?;
    let def = crate::services::def(account.service);
    // Only the service's own CDNs: a page can't make Mnemo fetch arbitrary addresses.
    if parsed.scheme() != "https"
        || !parsed
            .host_str()
            .map(|h| crate::services::host_matches(h, def.media_hosts))
            .unwrap_or(false)
    {
        return Ok(false);
    }
    let cap = rules::max_bytes(&s.rules_for(&account.id));
    let resp = state
        .http
        .get(parsed)
        .timeout(std::time::Duration::from_secs(90))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Ok(false);
    }
    if resp.content_length().map(|l| l > cap).unwrap_or(false) {
        return Ok(false);
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_string();
    let mut body = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if body.len() as u64 + chunk.len() as u64 > cap {
            return Ok(false);
        }
        body.extend_from_slice(&chunk);
    }
    save_live_media(&state, &account, &meta, &body, &mime)
}

/// Where account avatars are kept (one small data-URL file per account).
pub fn avatar_path(config_dir: &std::path::Path, account_id: &str) -> std::path::PathBuf {
    config_dir.join("avatars").join(format!(
        "{}.txt",
        crate::util::safe_component(account_id, 40)
    ))
}

pub fn set_avatar(
    app: &AppHandle,
    state: &AppState,
    account_id: &str,
    data_url: Option<String>,
) -> Result<(), String> {
    let path = avatar_path(&state.config_dir, account_id);
    match &data_url {
        Some(d) => crate::util::write_atomic(&path, d.as_bytes()).map_err(|e| e.to_string())?,
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
    {
        let mut a = lock(&state.avatars);
        match &data_url {
            Some(d) => a.insert(account_id.to_string(), d.clone()),
            None => a.remove(account_id),
        };
    }
    let _ = app.emit_to(
        SHELL,
        "avatar",
        json!({ "account": account_id, "avatar": data_url }),
    );
    Ok(())
}

/// Turns image bytes into a data URL, if they really are a small image.
pub fn image_data_url(bytes: &[u8], mime_hint: &str) -> Option<String> {
    if bytes.len() > 1_500_000 || bytes.len() < 32 {
        return None;
    }
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if mime_hint == "image/avif" {
        "image/avif"
    } else {
        return None;
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Called by an app's page with the signed-in person's own avatar (detected automatically).
/// A picture the person chose by hand is never replaced.
#[tauri::command]
pub async fn mnemo_profile(
    app: AppHandle,
    webview: Webview,
    state: State<'_, AppState>,
    avatar_data: Option<String>,
    avatar_url: Option<String>,
) -> Result<(), String> {
    let s = state.settings();
    let account = account_for(&webview, &s)?;
    if state
        .config_dir
        .join("avatars")
        .join(format!("{}.manual", account.id))
        .exists()
    {
        return Ok(());
    }
    let bytes = if let Some(d) = avatar_data {
        let b64 = d
            .split_once(";base64,")
            .map(|(_, b)| b)
            .ok_or("bad avatar")?;
        if b64.len() > 2_000_000 {
            return Ok(());
        }
        base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|_| "bad avatar")?
    } else if let Some(u) = avatar_url {
        let parsed = url::Url::parse(&u).map_err(|_| "bad url")?;
        let def = crate::services::def(account.service);
        if parsed.scheme() != "https"
            || !parsed
                .host_str()
                .map(|h| crate::services::host_matches(h, def.media_hosts))
                .unwrap_or(false)
        {
            return Ok(());
        }
        let resp = state
            .http
            .get(parsed)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() || resp.content_length().unwrap_or(0) > 1_500_000 {
            return Ok(());
        }
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else {
        return Ok(());
    };
    let Some(data_url) = image_data_url(&bytes, "") else {
        return Ok(());
    };
    if lock(&state.avatars).get(&account.id) == Some(&data_url) {
        return Ok(());
    }
    set_avatar(&app, &state, &account.id, Some(data_url))
}
