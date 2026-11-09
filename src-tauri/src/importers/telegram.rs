//! Telegram Desktop export ("Export chat history" / "Export Telegram data", JSON format).

use super::{AttachmentSource, ParsedAttachment, ParsedChat, ParsedMessage};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Finds `result.json` from a file or a folder the person picked.
fn locate(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let candidate = path.join("result.json");
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err("Không tìm thấy result.json. Hãy xuất từ Telegram Desktop ở định dạng JSON (Machine-readable JSON).".into())
}

/// Same chat ids as Telegram Web A uses in its URL, so live and imported messages meet.
fn chat_key(kind: &str, id: i64) -> String {
    match kind {
        "private_group" => format!("-{id}"),
        "private_supergroup" | "public_supergroup" | "private_channel" | "public_channel" => {
            format!("-100{id}")
        }
        _ => id.to_string(),
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| match p {
                Value::String(s) => s.clone(),
                Value::Object(o) => o
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            })
            .collect(),
        _ => String::new(),
    }
}

fn ts_of(m: &Value) -> Option<i64> {
    if let Some(s) = m.get("date_unixtime").and_then(|v| v.as_str()) {
        if let Ok(secs) = s.parse::<i64>() {
            return Some(secs * 1000);
        }
    }
    let d = m.get("date")?.as_str()?;
    let naive = chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S").ok()?;
    naive
        .and_local_timezone(chrono::Local)
        .single()
        .map(|t| t.timestamp_millis())
}

fn kind_of(media_type: Option<&str>, has_photo: bool) -> &'static str {
    if has_photo {
        return "image";
    }
    match media_type {
        Some("voice_message") | Some("audio_file") => "audio",
        Some("video_file") | Some("video_message") | Some("animation") => "video",
        Some("sticker") => "image",
        _ => "document",
    }
}

fn parse_chat(
    chat: &Value,
    root: &Path,
    self_id: Option<&str>,
    self_name: &str,
) -> Option<ParsedChat> {
    let id = chat.get("id")?.as_i64()?;
    let kind = chat.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let title = chat
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if kind == "saved_messages" {
                "Saved Messages".into()
            } else {
                id.to_string()
            }
        });
    let is_group = !matches!(kind, "personal_chat" | "bot_chat" | "saved_messages");
    let mut messages = vec![];
    for m in chat
        .get("messages")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let Some(mid) = m.get("id").and_then(|v| v.as_i64()) else {
            continue;
        };
        let Some(ts) = ts_of(m) else { continue };
        let service = m.get("type").and_then(|v| v.as_str()) == Some("service");
        let sender = m
            .get(if service { "actor" } else { "from" })
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let from_id = m
            .get(if service { "actor_id" } else { "from_id" })
            .and_then(|v| v.as_str());
        let outgoing = match (self_id, from_id) {
            (Some(me), Some(f)) => Some(f == me),
            _ if !self_name.is_empty() => sender.as_deref().map(|s| s == self_name),
            _ => None,
        };
        let mut text = text_of(m.get("text").unwrap_or(&Value::Null));
        if service && text.is_empty() {
            if let Some(a) = m.get("action").and_then(|v| v.as_str()) {
                text = format!("[{a}]");
            }
        }
        let media_type = m.get("media_type").and_then(|v| v.as_str());
        let mut attachments = vec![];
        for field in ["photo", "file"] {
            let Some(rel) = m.get(field).and_then(|v| v.as_str()) else {
                continue;
            };
            // "(File not included. Change data exporting settings to download.)"
            if rel.starts_with('(') || rel.is_empty() {
                continue;
            }
            let full = root.join(rel);
            let size = std::fs::metadata(&full).map(|md| md.len()).unwrap_or(0);
            let name = m
                .get("file_name")
                .and_then(|v| v.as_str())
                .filter(|_| field == "file")
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    Path::new(rel)
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            attachments.push(ParsedAttachment {
                name,
                kind: kind_of(media_type, field == "photo").into(),
                size,
                source: AttachmentSource::File(full),
            });
        }
        let kind = if service {
            "service"
        } else if attachments.is_empty() && media_type.is_none() && m.get("photo").is_none() {
            "text"
        } else if text.is_empty() {
            "media"
        } else {
            "text+media"
        };
        messages.push(ParsedMessage {
            key: mid.to_string(),
            sender,
            outgoing,
            ts,
            text,
            kind: kind.into(),
            reply_to: m
                .get("reply_to_message_id")
                .and_then(|v| v.as_i64())
                .map(|r| r.to_string()),
            attachments,
        });
    }
    Some(ParsedChat {
        key: chat_key(kind, id),
        title,
        is_group,
        messages,
    })
}

pub fn parse(
    path: &Path,
    self_name: &str,
    sink: &mut dyn FnMut(ParsedChat) -> Result<(), String>,
) -> Result<(), String> {
    let file = locate(path)?;
    let root = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let reader = std::io::BufReader::new(
        std::fs::File::open(&file).map_err(|e| format!("Không mở được {}: {e}", file.display()))?,
    );
    let v: Value =
        serde_json::from_reader(reader).map_err(|e| format!("result.json không hợp lệ: {e}"))?;
    let self_id = v
        .get("personal_information")
        .and_then(|p| p.get("user_id"))
        .and_then(|u| u.as_i64())
        .map(|u| format!("user{u}"));
    if v.get("messages").is_some() {
        if let Some(c) = parse_chat(&v, &root, self_id.as_deref(), self_name) {
            sink(c)?;
        }
        return Ok(());
    }
    let mut found = false;
    for section in ["chats", "left_chats"] {
        for chat in v
            .get(section)
            .and_then(|s| s.get("list"))
            .and_then(|l| l.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(c) = parse_chat(chat, &root, self_id.as_deref(), self_name) {
                found = true;
                sink(c)?;
            }
        }
    }
    if !found {
        return Err("Tệp xuất không có hội thoại nào.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_export() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("chats/chat_001/photos")).unwrap();
        std::fs::write(dir.path().join("chats/chat_001/photos/photo_1.jpg"), b"jpg").unwrap();
        let json = r#"{
          "personal_information": {"user_id": 42, "first_name": "Minh"},
          "chats": {"list": [
            {"name": "Lan", "type": "personal_chat", "id": 777, "messages": [
              {"id": 1, "type": "message", "date": "2026-11-09T10:30:00", "date_unixtime": "1762659000",
               "from": "Lan", "from_id": "user777", "text": "Chào anh"},
              {"id": 2, "type": "message", "date_unixtime": "1762659060", "from": "Minh", "from_id": "user42",
               "text": [{"type": "bold", "text": "OK"}, " nhé"], "photo": "chats/chat_001/photos/photo_1.jpg"},
              {"id": 3, "type": "message", "date_unixtime": "1762659070", "from": "Lan", "from_id": "user777",
               "file": "(File not included. Change data exporting settings to download.)", "media_type": "video_file", "text": ""}
            ]},
            {"name": "Dev VN", "type": "public_supergroup", "id": 1234, "messages": []}
          ]}
        }"#;
        std::fs::write(dir.path().join("result.json"), json).unwrap();
        let mut chats = vec![];
        parse(dir.path(), "", &mut |c| {
            chats.push(c);
            Ok(())
        })
        .unwrap();
        assert_eq!(chats.len(), 2);
        let lan = &chats[0];
        assert_eq!(lan.key, "777");
        assert!(!lan.is_group);
        assert_eq!(lan.messages[0].outgoing, Some(false));
        assert_eq!(lan.messages[1].outgoing, Some(true));
        assert_eq!(lan.messages[1].text, "OK nhé");
        assert_eq!(lan.messages[1].ts, 1_762_659_060_000);
        assert_eq!(lan.messages[1].attachments.len(), 1);
        assert_eq!(lan.messages[1].attachments[0].size, 3);
        assert!(
            lan.messages[2].attachments.is_empty(),
            "files left out of the export are skipped"
        );
        assert_eq!(chats[1].key, "-1001234");
        assert!(chats[1].is_group);
    }

    #[test]
    fn single_chat_export_uses_self_name() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{"name": "Mẹ", "type": "personal_chat", "id": 5, "messages": [
            {"id": 10, "type": "message", "date_unixtime": "1762659000", "from": "Minh", "from_id": "user1", "text": "Con về rồi"}]}"#;
        std::fs::write(dir.path().join("result.json"), json).unwrap();
        let mut chats = vec![];
        parse(&dir.path().join("result.json"), "Minh", &mut |c| {
            chats.push(c);
            Ok(())
        })
        .unwrap();
        assert_eq!(chats[0].messages[0].outgoing, Some(true));
    }
}
