//! Meta "Download your information" (JSON format) for Messenger and Instagram:
//! `.../messages/inbox/<thread>/message_1.json`, as a folder or the downloaded .zip.

use super::{AttachmentSource, ParsedAttachment, ParsedChat, ParsedMessage};
use crate::settings::ServiceId;
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Meta writes UTF-8 bytes as if each were a Latin-1 character ("Ä\u{91}" for "đ").
/// Undo that when the string is made only of such characters and decodes as UTF-8.
pub fn fix_mojibake(s: &str) -> String {
    if s.chars().all(|c| (c as u32) < 256) && s.chars().any(|c| (c as u32) >= 128) {
        let bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
        if let Ok(fixed) = String::from_utf8(bytes) {
            return fixed;
        }
    }
    s.to_string()
}

fn fix_value(v: &mut Value) {
    match v {
        Value::String(s) => *s = fix_mojibake(s),
        Value::Array(a) => a.iter_mut().for_each(fix_value),
        Value::Object(o) => o.values_mut().for_each(fix_value),
        _ => {}
    }
}

/// Every file of the export, by its path relative to the export root (with '/').
enum Source {
    Folder(PathBuf),
    Zip(PathBuf),
}

struct Index {
    source: Source,
    files: BTreeMap<String, u64>,
}

impl Index {
    fn build(path: &Path) -> Result<Index, String> {
        let mut files = BTreeMap::new();
        if path.is_dir() {
            fn walk(root: &Path, dir: &Path, depth: u32, out: &mut BTreeMap<String, u64>) {
                if depth > 10 {
                    return;
                }
                for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(root, &p, depth + 1, out);
                    } else if let Ok(rel) = p.strip_prefix(root) {
                        let rel = rel.to_string_lossy().replace('\\', "/");
                        out.insert(rel, e.metadata().map(|m| m.len()).unwrap_or(0));
                    }
                }
            }
            walk(path, path, 0, &mut files);
            Ok(Index {
                source: Source::Folder(path.to_path_buf()),
                files,
            })
        } else {
            let f = std::fs::File::open(path)
                .map_err(|e| format!("Không mở được {}: {e}", path.display()))?;
            let mut zip =
                zip::ZipArchive::new(f).map_err(|e| format!("Tệp zip không hợp lệ: {e}"))?;
            for i in 0..zip.len() {
                if let Ok(f) = zip.by_index(i) {
                    if !f.is_dir() {
                        if let Ok(n) = f.name() {
                            files.insert(n.to_string(), f.size());
                        }
                    }
                }
            }
            Ok(Index {
                source: Source::Zip(path.to_path_buf()),
                files,
            })
        }
    }

    fn read_json(
        &self,
        rel: &str,
        zip: &mut Option<zip::ZipArchive<std::fs::File>>,
    ) -> Result<Value, String> {
        let mut text = String::new();
        match &self.source {
            Source::Folder(root) => {
                text =
                    std::fs::read_to_string(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
            }
            Source::Zip(path) => {
                if zip.is_none() {
                    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
                    *zip = Some(zip::ZipArchive::new(f).map_err(|e| e.to_string())?);
                }
                let z = zip.as_mut().expect("opened above");
                z.by_name(rel)
                    .map_err(|e| format!("{rel}: {e}"))?
                    .read_to_string(&mut text)
                    .map_err(|e| format!("{rel}: {e}"))?;
            }
        }
        serde_json::from_str(&text).map_err(|e| format!("{rel}: {e}"))
    }

    /// Finds a media `uri` from the JSON among the export's files.
    fn resolve(&self, uri: &str) -> Option<(AttachmentSource, u64)> {
        let uri = uri.trim_start_matches("./").replace('\\', "/");
        let hit = self
            .files
            .get_key_value(&uri)
            .or_else(|| {
                self.files
                    .iter()
                    .find(|(k, _)| k.ends_with(&format!("/{uri}")))
            })
            .or_else(|| {
                // Older exports point at "messages/inbox/…" while files sit under "your_…_activity/messages/inbox/…".
                let tail = uri
                    .split_once("messages/")
                    .map(|(_, t)| format!("messages/{t}"))?;
                self.files.iter().find(|(k, _)| k.ends_with(&tail))
            })?;
        let (rel, size) = hit;
        let src = match &self.source {
            Source::Folder(root) => AttachmentSource::File(root.join(rel)),
            Source::Zip(path) => AttachmentSource::Zip {
                archive: path.clone(),
                entry: rel.clone(),
            },
        };
        Some((src, *size))
    }
}

fn service_of(rel: &str) -> Option<ServiceId> {
    if rel.contains("your_instagram_activity") {
        Some(ServiceId::Instagram)
    } else if rel.contains("your_facebook_activity") {
        Some(ServiceId::Messenger)
    } else {
        None
    }
}

pub fn parse(
    path: &Path,
    target: ServiceId,
    self_name: &str,
    sink: &mut dyn FnMut(ParsedChat) -> Result<(), String>,
) -> Result<(), String> {
    let index = Index::build(path)?;
    let thread_re = Regex::new(r"(?:^|/)messages/(inbox|archived_threads|filtered_threads|message_requests|e2ee_cutover)/([^/]+)/message_\d+\.json$").unwrap();
    // thread folder -> its message_N.json files
    let mut threads: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in index.files.keys() {
        if thread_re.is_match(rel) {
            if let Some(s) = service_of(rel) {
                if s != target {
                    continue;
                }
            }
            let folder = rel
                .rsplit_once('/')
                .map(|(f, _)| f.to_string())
                .unwrap_or_default();
            threads.entry(folder).or_default().push(rel.clone());
        }
    }
    if threads.is_empty() {
        return Err(format!(
            "Không tìm thấy tin nhắn {} trong bản xuất. Hãy tải \"Thông tin của bạn\" ở định dạng JSON và chọn mục Tin nhắn.",
            target.display()
        ));
    }
    let mut zip = None;
    for (folder, files) in threads {
        let thread_name = folder.rsplit('/').next().unwrap_or(&folder).to_string();
        let mut title = String::new();
        let mut participants = 0usize;
        let mut group_hint = false;
        let mut raw_messages: Vec<Value> = vec![];
        for f in files {
            let mut v = index.read_json(&f, &mut zip)?;
            fix_value(&mut v);
            if title.is_empty() {
                title = v
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            participants = participants.max(
                v.get("participants")
                    .and_then(|p| p.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0),
            );
            group_hint |= v
                .get("thread_type")
                .and_then(|t| t.as_str())
                .map(|t| t.contains("Group"))
                .unwrap_or(false);
            if let Some(Value::Array(ms)) = v.get_mut("messages").map(|m| m.take()) {
                raw_messages.extend(ms);
            }
        }
        let mut occurrences: HashMap<String, u32> = HashMap::new();
        let mut messages = vec![];
        for m in &raw_messages {
            let Some(ts) = m.get("timestamp_ms").and_then(|t| t.as_i64()) else {
                continue;
            };
            let sender = m
                .get("sender_name")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let mut text = m
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(share) = m.get("share") {
                if let Some(link) = share.get("link").and_then(|l| l.as_str()) {
                    if !text.contains(link) {
                        text = if text.is_empty() {
                            link.to_string()
                        } else {
                            format!("{text}\n{link}")
                        };
                    }
                }
            }
            let mut attachments = vec![];
            for (field, kind) in [
                ("photos", "image"),
                ("videos", "video"),
                ("audio_files", "audio"),
                ("files", "document"),
                ("gifs", "image"),
            ] {
                for item in m
                    .get(field)
                    .and_then(|a| a.as_array())
                    .into_iter()
                    .flatten()
                {
                    let Some(uri) = item.get("uri").and_then(|u| u.as_str()) else {
                        continue;
                    };
                    if uri.starts_with("http") {
                        continue;
                    }
                    if let Some((source, size)) = index.resolve(uri) {
                        let name = uri.rsplit('/').next().unwrap_or(uri).to_string();
                        attachments.push(ParsedAttachment {
                            name,
                            kind: kind.into(),
                            size,
                            source,
                        });
                    }
                }
            }
            let base = format!("{ts}|{}|{}", sender.as_deref().unwrap_or(""), text);
            let n = occurrences.entry(base.clone()).or_insert(0);
            *n += 1;
            let unsent = m
                .get("is_unsent")
                .and_then(|u| u.as_bool())
                .unwrap_or(false);
            let kind = if unsent {
                "unsent"
            } else if attachments.is_empty() {
                "text"
            } else if text.is_empty() {
                "media"
            } else {
                "text+media"
            };
            messages.push(ParsedMessage {
                key: format!(
                    "dyi:{}",
                    &crate::util::sha256_hex(format!("{base}|{n}").as_bytes())[..24]
                ),
                outgoing: if self_name.is_empty() {
                    None
                } else {
                    sender.as_deref().map(|s| s == self_name)
                },
                sender,
                ts,
                text,
                kind: kind.into(),
                reply_to: None,
                attachments,
            });
        }
        messages.sort_by_key(|m| m.ts);
        if title.is_empty() {
            title = thread_name.clone();
        }
        sink(ParsedChat {
            key: format!("dyi:{thread_name}"),
            title,
            is_group: group_hint || participants > 2,
            messages,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojibake() {
        // "Đi ăn phở" as Meta exports it.
        let broken: String = "Đi ăn phở".as_bytes().iter().map(|b| *b as char).collect();
        assert_eq!(fix_mojibake(&broken), "Đi ăn phở");
        assert_eq!(fix_mojibake("plain"), "plain");
        assert_eq!(
            fix_mojibake("café"),
            "café",
            "real Latin-1 text that isn't UTF-8 stays"
        );
    }

    #[test]
    fn messenger_folder_export() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir
            .path()
            .join("your_facebook_activity/messages/inbox/lan_123");
        std::fs::create_dir_all(t.join("photos")).unwrap();
        std::fs::write(t.join("photos/1.jpg"), b"jpg").unwrap();
        let name: String = "Lan Phạm".as_bytes().iter().map(|b| *b as char).collect();
        let json = serde_json::json!({
            "participants": [{"name": name}, {"name": "Minh"}],
            "title": name,
            "messages": [
                {"sender_name": "Minh", "timestamp_ms": 2000, "content": "ok",
                 "photos": [{"uri": "your_facebook_activity/messages/inbox/lan_123/photos/1.jpg"}]},
                {"sender_name": name, "timestamp_ms": 1000, "content": "hi"}
            ]
        });
        std::fs::write(t.join("message_1.json"), json.to_string()).unwrap();
        // An Instagram thread in the same export must not leak into Messenger.
        let ig = dir
            .path()
            .join("your_instagram_activity/messages/inbox/x_1");
        std::fs::create_dir_all(&ig).unwrap();
        std::fs::write(ig.join("message_1.json"), r#"{"participants":[],"messages":[{"sender_name":"a","timestamp_ms":1,"content":"x"}]}"#).unwrap();
        let mut chats = vec![];
        parse(dir.path(), ServiceId::Messenger, "Minh", &mut |c| {
            chats.push(c);
            Ok(())
        })
        .unwrap();
        assert_eq!(chats.len(), 1);
        let c = &chats[0];
        assert_eq!(c.title, "Lan Phạm");
        assert_eq!(c.messages[0].text, "hi", "sorted oldest first");
        assert_eq!(c.messages[0].sender.as_deref(), Some("Lan Phạm"));
        assert_eq!(c.messages[1].outgoing, Some(true));
        assert_eq!(c.messages[1].attachments.len(), 1);
    }
}
