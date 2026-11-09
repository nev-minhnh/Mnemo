//! Importing official exports (level B): Telegram Desktop JSON, WhatsApp chat exports,
//! and Meta "Download your information" for Messenger and Instagram.

pub mod meta;
pub mod telegram;
pub mod whatsapp;

use crate::archive::NewMessage;
use crate::media;
use crate::rules::{self, Candidate};
use crate::settings::ServiceId;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum AttachmentSource {
    File(PathBuf),
    Zip { archive: PathBuf, entry: String },
}

#[derive(Clone, Debug)]
pub struct ParsedAttachment {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub source: AttachmentSource,
}

#[derive(Clone, Debug)]
pub struct ParsedMessage {
    pub key: String,
    pub sender: Option<String>,
    pub outgoing: Option<bool>,
    pub ts: i64,
    pub text: String,
    pub kind: String,
    pub reply_to: Option<String>,
    pub attachments: Vec<ParsedAttachment>,
}

#[derive(Clone, Debug)]
pub struct ParsedChat {
    pub key: String,
    pub title: String,
    pub is_group: bool,
    pub messages: Vec<ParsedMessage>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    Telegram,
    Whatsapp,
    Meta,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ImportReport {
    pub chats: u64,
    pub messages_new: u64,
    pub messages_updated: u64,
    pub media_saved: u64,
    pub media_skipped: u64,
    pub errors: Vec<String>,
}

impl ImportReport {
    fn error(&mut self, e: String) {
        if self.errors.len() < 50 {
            self.errors.push(e);
        }
    }
}

/// Reads an attachment, refusing anything larger than `cap`.
fn read_attachment(
    src: &AttachmentSource,
    cap: u64,
    zips: &mut HashMap<PathBuf, zip::ZipArchive<std::fs::File>>,
) -> Result<Vec<u8>, String> {
    match src {
        AttachmentSource::File(p) => {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let mut buf = Vec::new();
            f.take(cap + 1)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            if buf.len() as u64 > cap {
                return Err("too large".into());
            }
            Ok(buf)
        }
        AttachmentSource::Zip { archive, entry } => {
            if !zips.contains_key(archive) {
                let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
                zips.insert(
                    archive.clone(),
                    zip::ZipArchive::new(f).map_err(|e| e.to_string())?,
                );
            }
            let z = zips.get_mut(archive).expect("inserted above");
            let mut file = z.by_name(entry).map_err(|e| format!("{entry}: {e}"))?;
            let mut buf = Vec::new();
            (&mut file)
                .take(cap + 1)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            if buf.len() as u64 > cap {
                return Err("too large".into());
            }
            Ok(buf)
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub chats: u64,
    pub messages: u64,
    pub media: u64,
    pub current: String,
}

/// Runs an import into an account's archive. `progress` is called after every chat.
pub fn run(
    state: &AppState,
    kind: ImportKind,
    path: &Path,
    account_id: &str,
    progress: &dyn Fn(Progress),
) -> Result<ImportReport, String> {
    let settings = state.settings();
    let account = settings
        .account(account_id)
        .cloned()
        .ok_or("Không tìm thấy tài khoản")?;
    let ok = match kind {
        ImportKind::Telegram => account.service == ServiceId::Telegram,
        ImportKind::Whatsapp => account.service == ServiceId::Whatsapp,
        ImportKind::Meta => matches!(account.service, ServiceId::Messenger | ServiceId::Instagram),
    };
    if !ok {
        return Err(format!(
            "Tệp xuất này không dành cho tài khoản {}",
            account.service.display()
        ));
    }
    let key = state.key()?;
    let rules_ = settings.rules_for(account_id);
    let encrypt = settings.encrypt_media;
    let cap = rules::max_bytes(&rules_);
    let mut report = ImportReport::default();
    let mut zips = HashMap::new();
    let now = crate::util::now_ms();

    let mut sink = |chat: ParsedChat| -> Result<(), String> {
        let msgs: Vec<NewMessage> = chat
            .messages
            .iter()
            .map(|m| NewMessage {
                key: m.key.clone(),
                sender: m.sender.clone(),
                outgoing: m.outgoing,
                ts: Some(m.ts),
                time_text: None,
                text: m.text.clone(),
                kind: m.kind.clone(),
                reply_to: m.reply_to.clone(),
                source: format!("import:{}", account.service.as_str()),
                recalled: false,
                prev_key: None,
                next_key: None,
            })
            .collect();
        let written = state.with_archive(|a| {
            a.transaction(|a| {
                let chat_id = a.upsert_chat(
                    account.service.as_str(),
                    &account.id,
                    &chat.key,
                    Some(&chat.title),
                    Some(chat.is_group),
                )?;
                let mut out = Vec::with_capacity(msgs.len());
                // Large chats are written in slices so each statement stays small.
                for slice in msgs.chunks(2000) {
                    let (r, _, _) = a.write_batch(chat_id, slice, now, true)?;
                    out.extend(r);
                }
                Ok((chat_id, out))
            })
        });
        let (chat_id, results) = match written {
            Ok(v) => v,
            Err(e) => {
                report.error(format!("{}: {e}", chat.title));
                return Ok(());
            }
        };
        report.chats += 1;
        for (_, o) in &results {
            match o {
                crate::archive::Outcome::Inserted => report.messages_new += 1,
                crate::archive::Outcome::Updated => report.messages_updated += 1,
                _ => {}
            }
        }
        for (m, (message_id, _)) in chat.messages.iter().zip(results.iter()) {
            for att in &m.attachments {
                let ext = crate::util::ext_of(&att.name);
                let cand = Candidate {
                    ext: &ext,
                    mime: "",
                    size: att.size,
                    is_group: chat.is_group,
                    chat_key: &chat.key,
                    chat_title: &chat.title,
                };
                if rules::decide(&rules_, &cand).is_err() {
                    report.media_skipped += 1;
                    continue;
                }
                let data = match read_attachment(&att.source, cap, &mut zips) {
                    Ok(d) => d,
                    Err(_) => {
                        report.media_skipped += 1;
                        continue;
                    }
                };
                let sha = crate::util::sha256_hex(&data);
                let saved = (|| -> Result<bool, String> {
                    if state.with_archive(|a| a.media_linked(*message_id, &sha))? {
                        return Ok(false);
                    }
                    let path = match state.with_archive(|a| a.file_by_hash(&sha, encrypt))? {
                        Some(p) => p,
                        None => {
                            let dir = media::chat_dir(
                                &settings.downloads_dir,
                                account.service,
                                &account.label,
                                &chat.title,
                                &chat.key,
                                m.ts,
                            );
                            media::write_new(&dir, &att.name, &data, encrypt.then_some(&key))?.path
                        }
                    };
                    state.with_archive(|a| {
                        a.add_media(&crate::archive::NewMedia {
                            message_id: Some(*message_id),
                            chat_id: Some(chat_id),
                            kind: rules::categorize(&ext, "").as_str(),
                            file_name: &att.name,
                            mime: "",
                            size: data.len() as i64,
                            sha256: &sha,
                            path: &path,
                            encrypted: encrypt,
                            origin: "import",
                        })
                    })?;
                    Ok(true)
                })();
                match saved {
                    Ok(true) => report.media_saved += 1,
                    Ok(false) => {}
                    Err(e) => {
                        report.media_skipped += 1;
                        report.error(format!("{}: {e}", att.name));
                    }
                }
            }
        }
        progress(Progress {
            chats: report.chats,
            messages: report.messages_new + report.messages_updated,
            media: report.media_saved,
            current: chat.title.clone(),
        });
        Ok(())
    };

    match kind {
        ImportKind::Telegram => telegram::parse(path, &account.self_name, &mut sink)?,
        ImportKind::Whatsapp => whatsapp::parse(path, &account.self_name, &mut sink)?,
        ImportKind::Meta => meta::parse(path, account.service, &account.self_name, &mut sink)?,
    }
    Ok(report)
}
