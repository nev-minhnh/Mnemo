//! The encrypted message archive: one SQLCipher database (`mnemo.db`) in the folder the
//! person chose, with full-text search that ignores Vietnamese diacritics.

use crate::crypto::Key;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const DB_NAME: &str = "mnemo.db";
const SCHEMA_VERSION: i64 = 1;

pub struct Archive {
    conn: Connection,
    path: PathBuf,
}

/// A message as it arrives from live capture or an importer.
#[derive(Clone, Debug, Default)]
pub struct NewMessage {
    pub key: String,
    pub sender: Option<String>,
    pub outgoing: Option<bool>,
    /// Exact time (ms) when the source gives one.
    pub ts: Option<i64>,
    /// Time as displayed by the app, when it isn't exact ("10:30").
    pub time_text: Option<String>,
    pub text: String,
    pub kind: String,
    pub reply_to: Option<String>,
    pub source: String,
    pub recalled: bool,
    /// Keys of the neighbours in the app's list (live capture), to keep the order right.
    pub prev_key: Option<String>,
    pub next_key: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Outcome {
    Inserted,
    Updated,
    Unchanged,
}

#[derive(Serialize, Clone, Debug)]
pub struct ChatRow {
    pub id: i64,
    pub service: String,
    pub account: String,
    pub chat_key: String,
    pub title: String,
    pub is_group: bool,
    pub last_ts: i64,
    pub count: i64,
    pub unseen: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct MediaRow {
    pub id: i64,
    pub message_id: Option<i64>,
    pub chat_id: Option<i64>,
    pub kind: String,
    pub file_name: String,
    pub mime: String,
    pub size: i64,
    pub path: String,
    pub encrypted: bool,
    pub origin: String,
    pub saved_at: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct MessageRow {
    pub id: i64,
    pub chat_id: i64,
    pub key: String,
    pub sender: Option<String>,
    pub outgoing: Option<bool>,
    pub ts: i64,
    pub ts_exact: bool,
    pub time_text: Option<String>,
    pub ord: i64,
    pub text: String,
    pub kind: String,
    pub source: String,
    pub deleted: bool,
    pub edited: bool,
    pub media: Vec<MediaRow>,
}

#[derive(Serialize, Clone, Debug)]
pub struct SearchHit {
    pub message_id: i64,
    pub chat_id: i64,
    pub chat_title: String,
    pub service: String,
    pub account: String,
    pub sender: Option<String>,
    pub ts: i64,
    pub snippet: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Stats {
    pub chats: i64,
    pub messages: i64,
    pub media: i64,
    pub db_bytes: u64,
}

pub struct NewMedia<'a> {
    pub message_id: Option<i64>,
    pub chat_id: Option<i64>,
    pub kind: &'a str,
    pub file_name: &'a str,
    pub mime: &'a str,
    pub size: i64,
    pub sha256: &'a str,
    pub path: &'a Path,
    pub encrypted: bool,
    pub origin: &'a str,
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

impl Archive {
    pub fn db_path(dir: &Path) -> PathBuf {
        dir.join(DB_NAME)
    }

    /// Opens (or creates) the archive in `dir`. Fails with a clear message when the key is wrong.
    pub fn open(dir: &Path, key: &Key) -> Result<Archive, String> {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Không tạo được thư mục lưu trữ {}: {e}", dir.display()))?;
        let path = Self::db_path(dir);
        let conn = Connection::open(&path).map_err(err)?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex::encode(key)))
            .map_err(err)?;
        // The first read is what fails when the key doesn't match.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|_| {
            "Không mở được kho lưu trữ: khóa không khớp hoặc tệp bị hỏng. Hãy nhập khóa khôi phục."
                .to_string()
        })?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;",
        )
        .map_err(err)?;
        let a = Archive { conn, path };
        a.migrate()?;
        Ok(a)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> Result<(), String> {
        let v: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(err)?;
        if v >= SCHEMA_VERSION {
            return Ok(());
        }
        self.conn
            .execute_batch(
                r#"
BEGIN;
CREATE TABLE IF NOT EXISTS chats(
  id INTEGER PRIMARY KEY,
  service TEXT NOT NULL,
  account TEXT NOT NULL,
  chat_key TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  is_group INTEGER NOT NULL DEFAULT 0,
  last_ts INTEGER NOT NULL DEFAULT 0,
  last_read_ord INTEGER NOT NULL DEFAULT 0,
  UNIQUE(service, account, chat_key)
);
CREATE TABLE IF NOT EXISTS messages(
  id INTEGER PRIMARY KEY,
  chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  msg_key TEXT NOT NULL,
  sender TEXT,
  outgoing INTEGER,
  ts INTEGER NOT NULL,
  ts_exact INTEGER NOT NULL DEFAULT 0,
  time_text TEXT,
  ord INTEGER NOT NULL,
  text TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'text',
  reply_to TEXT,
  source TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0,
  edited INTEGER NOT NULL DEFAULT 0,
  captured_at INTEGER NOT NULL,
  UNIQUE(chat_id, msg_key)
);
CREATE INDEX IF NOT EXISTS messages_chat_ord ON messages(chat_id, ord, id);
CREATE INDEX IF NOT EXISTS messages_chat_ts ON messages(chat_id, ts);
CREATE TABLE IF NOT EXISTS message_edits(
  id INTEGER PRIMARY KEY,
  message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  old_text TEXT NOT NULL,
  changed_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS media(
  id INTEGER PRIMARY KEY,
  message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL,
  chat_id INTEGER REFERENCES chats(id) ON DELETE SET NULL,
  kind TEXT NOT NULL,
  file_name TEXT NOT NULL,
  mime TEXT NOT NULL DEFAULT '',
  size INTEGER NOT NULL DEFAULT 0,
  sha256 TEXT NOT NULL DEFAULT '',
  path TEXT NOT NULL,
  encrypted INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL,
  saved_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS media_message ON media(message_id);
CREATE INDEX IF NOT EXISTS media_sha ON media(sha256);
CREATE INDEX IF NOT EXISTS media_saved ON media(saved_at);
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
  text, sender, content='messages', content_rowid='id',
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, text, sender) VALUES (new.id, new.text, coalesce(new.sender, ''));
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, text, sender) VALUES ('delete', old.id, old.text, coalesce(old.sender, ''));
END;
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE OF text, sender ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, text, sender) VALUES ('delete', old.id, old.text, coalesce(old.sender, ''));
  INSERT INTO messages_fts(rowid, text, sender) VALUES (new.id, new.text, coalesce(new.sender, ''));
END;
PRAGMA user_version = 1;
COMMIT;
"#,
            )
            .map_err(err)
    }

    /// Runs `f` in one transaction (imports write a chat at a time).
    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&Archive) -> Result<T, String>,
    ) -> Result<T, String> {
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
        match f(self) {
            Ok(v) => {
                self.conn.execute_batch("COMMIT").map_err(err)?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn upsert_chat(
        &self,
        service: &str,
        account: &str,
        chat_key: &str,
        title: Option<&str>,
        is_group: Option<bool>,
    ) -> Result<i64, String> {
        let title = title
            .map(|t| crate::util::truncate_chars(t.trim(), 300))
            .filter(|t| !t.is_empty());
        self.conn
            .execute(
                "INSERT INTO chats(service, account, chat_key, title, is_group) VALUES (?1, ?2, ?3, coalesce(?4, ''), coalesce(?5, 0))
                 ON CONFLICT(service, account, chat_key) DO UPDATE SET
                   title = coalesce(?4, chats.title),
                   is_group = coalesce(?5, chats.is_group)",
                params![service, account, chat_key, title, is_group],
            )
            .map_err(err)?;
        self.conn
            .query_row(
                "SELECT id FROM chats WHERE service = ?1 AND account = ?2 AND chat_key = ?3",
                params![service, account, chat_key],
                |r| r.get(0),
            )
            .map_err(err)
    }

    pub fn chat(&self, chat_id: i64) -> Result<Option<ChatRow>, String> {
        self.conn
            .query_row(
                &format!("{CHAT_SELECT} WHERE c.id = ?1"),
                params![chat_id],
                chat_row,
            )
            .optional()
            .map_err(err)
    }

    pub fn find_chat(
        &self,
        service: &str,
        account: &str,
        chat_key: &str,
    ) -> Result<Option<i64>, String> {
        self.conn
            .query_row(
                "SELECT id FROM chats WHERE service = ?1 AND account = ?2 AND chat_key = ?3",
                params![service, account, chat_key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)
    }

    fn ord_of(&self, chat_id: i64, key: &str) -> Result<Option<i64>, String> {
        self.conn
            .query_row(
                "SELECT ord FROM messages WHERE chat_id = ?1 AND msg_key = ?2",
                params![chat_id, key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)
    }

    pub fn message_id(&self, chat_id: i64, key: &str) -> Result<Option<i64>, String> {
        self.conn
            .query_row(
                "SELECT id FROM messages WHERE chat_id = ?1 AND msg_key = ?2",
                params![chat_id, key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)
    }

    /// Writes a batch of messages for one chat, in the app's display order.
    ///
    /// Messages without an exact time get an order from their neighbours: just after the
    /// one before them, or just before the one after them, so scrolling up to older messages
    /// keeps them above the newer ones. Returns the outcome per message, plus files of
    /// recalled messages that must be deleted.
    pub fn write_batch(
        &self,
        chat_id: i64,
        msgs: &[NewMessage],
        now: i64,
        keep_recalled: bool,
    ) -> Result<(Vec<(i64, Outcome)>, Vec<PathBuf>, bool), String> {
        let n = msgs.len();
        let mut ords: Vec<Option<i64>> = Vec::with_capacity(n);
        for m in msgs {
            let existing = self.ord_of(chat_id, &m.key)?;
            ords.push(existing.or(m.ts));
        }
        // Anchor runs on neighbours already in the archive.
        for i in 0..n {
            if ords[i].is_none() {
                if let Some(prev) = &msgs[i].prev_key {
                    if let Some(o) = self.ord_of(chat_id, prev)? {
                        ords[i] = Some(o + 1);
                    }
                }
            }
        }
        for i in 1..n {
            if ords[i].is_none() {
                if let Some(o) = ords[i - 1] {
                    ords[i] = Some(o + 1);
                }
            }
        }
        for i in (0..n).rev() {
            if ords[i].is_none() {
                let next_ord = if i + 1 < n {
                    ords[i + 1]
                } else {
                    match &msgs[i].next_key {
                        Some(k) => self.ord_of(chat_id, k)?,
                        None => None,
                    }
                };
                if let Some(o) = next_ord {
                    ords[i] = Some(o - 1);
                }
            }
        }
        let mut results = Vec::with_capacity(n);
        let mut files = Vec::new();
        let mut last_ts = 0i64;
        let mut read_ord: Option<i64> = None;
        let mut any_change = false;
        for (i, m) in msgs.iter().enumerate() {
            let ord = ords[i].unwrap_or(now + i as i64);
            let ts = m.ts.unwrap_or(now);
            last_ts = last_ts.max(ts);
            if m.outgoing == Some(true) {
                read_ord = Some(read_ord.map_or(ord, |r: i64| r.max(ord)));
            }
            if m.recalled {
                if let Some(id) = self.message_id(chat_id, &m.key)? {
                    files.extend(self.mark_recalled(id, keep_recalled)?);
                    results.push((id, Outcome::Updated));
                    any_change = true;
                    continue;
                }
            }
            let (id, outcome) = self.upsert_message(chat_id, m, ord, ts, now)?;
            any_change |= outcome != Outcome::Unchanged;
            results.push((id, outcome));
        }
        if last_ts > 0 {
            self.conn
                .execute(
                    "UPDATE chats SET last_ts = max(last_ts, ?2) WHERE id = ?1",
                    params![chat_id, last_ts],
                )
                .map_err(err)?;
        }
        if let Some(r) = read_ord {
            self.set_read_up_to(chat_id, r)?;
        }
        Ok((results, files, any_change))
    }

    fn upsert_message(
        &self,
        chat_id: i64,
        m: &NewMessage,
        ord: i64,
        ts: i64,
        now: i64,
    ) -> Result<(i64, Outcome), String> {
        let text = crate::util::truncate_chars(&m.text, 100_000);
        let existing: Option<(i64, String, i64)> = self
            .conn
            .query_row(
                "SELECT id, text, deleted FROM messages WHERE chat_id = ?1 AND msg_key = ?2",
                params![chat_id, m.key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(err)?;
        match existing {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO messages(chat_id, msg_key, sender, outgoing, ts, ts_exact, time_text, ord, text, kind, reply_to, source, captured_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                        params![
                            chat_id,
                            m.key,
                            m.sender,
                            m.outgoing,
                            ts,
                            m.ts.is_some(),
                            m.time_text,
                            ord,
                            text,
                            if m.kind.is_empty() { "text" } else { m.kind.as_str() },
                            m.reply_to,
                            m.source,
                            now
                        ],
                    )
                    .map_err(err)?;
                Ok((self.conn.last_insert_rowid(), Outcome::Inserted))
            }
            Some((id, old_text, deleted)) => {
                // Fill details a later source knows better, never blank them out.
                self.conn
                    .execute(
                        "UPDATE messages SET sender = coalesce(sender, ?2), outgoing = coalesce(outgoing, ?3),
                           ts = CASE WHEN ts_exact = 0 AND ?4 THEN ?5 ELSE ts END,
                           ts_exact = max(ts_exact, ?4) WHERE id = ?1",
                        params![id, m.sender, m.outgoing, m.ts.is_some(), ts],
                    )
                    .map_err(err)?;
                if deleted == 0 && !text.is_empty() && old_text != text {
                    if !old_text.is_empty() {
                        self.conn
                            .execute(
                                "INSERT INTO message_edits(message_id, old_text, changed_at) VALUES (?1, ?2, ?3)",
                                params![id, old_text, now],
                            )
                            .map_err(err)?;
                    }
                    self.conn
                        .execute(
                            "UPDATE messages SET text = ?2, edited = CASE WHEN ?3 = '' THEN edited ELSE 1 END WHERE id = ?1",
                            params![id, text, old_text],
                        )
                        .map_err(err)?;
                    return Ok((id, Outcome::Updated));
                }
                Ok((id, Outcome::Unchanged))
            }
        }
    }

    /// A sender recalled the message. Unless the person keeps recalled messages, its text,
    /// edit history and saved files go too. Returns files to delete from disk.
    fn mark_recalled(&self, id: i64, keep: bool) -> Result<Vec<PathBuf>, String> {
        if keep {
            self.conn
                .execute("UPDATE messages SET deleted = 1 WHERE id = ?1", params![id])
                .map_err(err)?;
            return Ok(vec![]);
        }
        let files = self.media_files_of(id)?;
        self.conn
            .execute(
                "DELETE FROM message_edits WHERE message_id = ?1",
                params![id],
            )
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM media WHERE message_id = ?1", params![id])
            .map_err(err)?;
        self.conn
            .execute(
                "UPDATE messages SET deleted = 1, text = '' WHERE id = ?1",
                params![id],
            )
            .map_err(err)?;
        Ok(self.unreferenced(files)?)
    }

    fn media_files_of(&self, message_id: i64) -> Result<Vec<PathBuf>, String> {
        let mut st = self
            .conn
            .prepare("SELECT path FROM media WHERE message_id = ?1")
            .map_err(err)?;
        let rows = st
            .query_map(params![message_id], |r| r.get::<_, String>(0))
            .map_err(err)?
            .filter_map(|r| r.ok())
            .map(PathBuf::from)
            .collect();
        Ok(rows)
    }

    /// Keeps only paths that no media row points to any more (files are shared by hash).
    fn unreferenced(&self, files: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
        let mut out = vec![];
        for f in files {
            let n: i64 = self
                .conn
                .query_row(
                    "SELECT count(*) FROM media WHERE path = ?1",
                    params![f.to_string_lossy()],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if n == 0 {
                out.push(f);
            }
        }
        Ok(out)
    }

    /// Everything up to `ord` counts as seen (only ever moves forward).
    pub fn set_read_up_to(&self, chat_id: i64, ord: i64) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE chats SET last_read_ord = max(last_read_ord, ?2) WHERE id = ?1",
                params![chat_id, ord],
            )
            .map(|_| ())
            .map_err(err)
    }

    pub fn set_read_up_to_key(&self, chat_id: i64, key: &str) -> Result<(), String> {
        if let Some(o) = self.ord_of(chat_id, key)? {
            self.set_read_up_to(chat_id, o)?;
        }
        Ok(())
    }

    pub fn mark_all_seen(&self, chat_id: i64) -> Result<(), String> {
        let max: Option<i64> = self
            .conn
            .query_row(
                "SELECT max(ord) FROM messages WHERE chat_id = ?1",
                params![chat_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if let Some(m) = max {
            self.set_read_up_to(chat_id, m)?;
        }
        Ok(())
    }

    pub fn add_media(&self, m: &NewMedia) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO media(message_id, chat_id, kind, file_name, mime, size, sha256, path, encrypted, origin, saved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    m.message_id,
                    m.chat_id,
                    m.kind,
                    m.file_name,
                    m.mime,
                    m.size,
                    m.sha256,
                    m.path.to_string_lossy(),
                    m.encrypted,
                    m.origin,
                    crate::util::now_ms()
                ],
            )
            .map_err(err)?;
        Ok(self.conn.last_insert_rowid())
    }

    /// A saved file with the same content, if there is one still on disk.
    pub fn file_by_hash(&self, sha: &str, encrypted: bool) -> Result<Option<PathBuf>, String> {
        let mut st = self
            .conn
            .prepare("SELECT path FROM media WHERE sha256 = ?1 AND encrypted = ?2 ORDER BY id DESC LIMIT 5")
            .map_err(err)?;
        let paths: Vec<String> = st
            .query_map(params![sha, encrypted], |r| r.get(0))
            .map_err(err)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(paths.into_iter().map(PathBuf::from).find(|p| p.exists()))
    }

    pub fn media_linked(&self, message_id: i64, sha: &str) -> Result<bool, String> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM media WHERE message_id = ?1 AND sha256 = ?2",
                params![message_id, sha],
                |r| r.get(0),
            )
            .map_err(err)?;
        Ok(n > 0)
    }

    pub fn list_chats(
        &self,
        service: Option<&str>,
        account: Option<&str>,
        query: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ChatRow>, String> {
        let q = query
            .map(|s| format!("%{}%", s.trim().to_lowercase()))
            .filter(|s| s != "%%");
        let mut st = self
            .conn
            .prepare(&format!(
                "{CHAT_SELECT} WHERE (?1 IS NULL OR c.service = ?1) AND (?2 IS NULL OR c.account = ?2)
                   AND (?3 IS NULL OR lower(c.title) LIKE ?3 OR lower(c.chat_key) LIKE ?3)
                 ORDER BY c.last_ts DESC, c.id DESC LIMIT ?4"
            ))
            .map_err(err)?;
        let rows = st
            .query_map(params![service, account, q, limit], chat_row)
            .map_err(err)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn load_media(&self, rows: &mut [MessageRow]) -> Result<(), String> {
        let mut st = self
            .conn
            .prepare(&format!("{MEDIA_SELECT} WHERE message_id = ?1 ORDER BY id"))
            .map_err(err)?;
        for m in rows.iter_mut() {
            m.media = st
                .query_map(params![m.id], media_row)
                .map_err(err)?
                .filter_map(|r| r.ok())
                .collect();
        }
        Ok(())
    }

    fn query_messages(
        &self,
        sql: &str,
        p: impl rusqlite::Params,
    ) -> Result<Vec<MessageRow>, String> {
        let mut st = self.conn.prepare(sql).map_err(err)?;
        let mut rows: Vec<MessageRow> = st
            .query_map(p, message_row)
            .map_err(err)?
            .filter_map(|r| r.ok())
            .collect();
        self.load_media(&mut rows)?;
        Ok(rows)
    }

    /// Newest `limit` messages before (ord, id), oldest first.
    pub fn messages(
        &self,
        chat_id: i64,
        before: Option<(i64, i64)>,
        limit: i64,
    ) -> Result<Vec<MessageRow>, String> {
        let (bo, bi) = before.unwrap_or((i64::MAX, i64::MAX));
        let mut rows = self.query_messages(
            &format!("{MSG_SELECT} WHERE chat_id = ?1 AND (ord < ?2 OR (ord = ?2 AND id < ?3)) ORDER BY ord DESC, id DESC LIMIT ?4"),
            params![chat_id, bo, bi, limit],
        )?;
        rows.reverse();
        Ok(rows)
    }

    /// Messages around one message (for opening a search hit), oldest first.
    pub fn messages_around(
        &self,
        chat_id: i64,
        message_id: i64,
        each_side: i64,
    ) -> Result<Vec<MessageRow>, String> {
        let (ord, id): (i64, i64) = self
            .conn
            .query_row(
                "SELECT ord, id FROM messages WHERE id = ?1",
                params![message_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        let mut before = self.messages(chat_id, Some((ord, id)), each_side)?;
        let after = self.query_messages(
            &format!("{MSG_SELECT} WHERE chat_id = ?1 AND (ord > ?2 OR (ord = ?2 AND id >= ?3)) ORDER BY ord, id LIMIT ?4"),
            params![chat_id, ord, id, each_side + 1],
        )?;
        before.extend(after);
        Ok(before)
    }

    /// Messages between two times (ms, inclusive), oldest first, capped.
    pub fn messages_in_range(
        &self,
        chat_id: i64,
        from: i64,
        to: i64,
        limit: i64,
    ) -> Result<(Vec<MessageRow>, bool), String> {
        let mut rows = self.query_messages(
            &format!("{MSG_SELECT} WHERE chat_id = ?1 AND ts >= ?2 AND ts <= ?3 AND deleted = 0 ORDER BY ord DESC, id DESC LIMIT ?4"),
            params![chat_id, from, to, limit + 1],
        )?;
        let truncated = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        rows.reverse();
        Ok((rows, truncated))
    }

    /// Incoming messages after the last one the person has seen, oldest first, capped.
    pub fn unseen_messages(
        &self,
        chat_id: i64,
        limit: i64,
    ) -> Result<(Vec<MessageRow>, bool), String> {
        let mut rows = self.query_messages(
            &format!(
                "{MSG_SELECT} WHERE chat_id = ?1 AND deleted = 0 AND coalesce(outgoing, 0) = 0
                   AND ord > (SELECT last_read_ord FROM chats WHERE id = ?1)
                 ORDER BY ord DESC, id DESC LIMIT ?2"
            ),
            params![chat_id, limit + 1],
        )?;
        let truncated = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        rows.reverse();
        Ok((rows, truncated))
    }

    pub fn search(
        &self,
        query: &str,
        service: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SearchHit>, String> {
        let fts = fts_query(query);
        if fts.is_empty() {
            return Ok(vec![]);
        }
        let mut st = self
            .conn
            .prepare(
                "SELECT m.id, m.chat_id, c.title, c.service, c.account, m.sender, m.ts,
                        snippet(messages_fts, 0, '[[', ']]', '…', 16)
                 FROM messages_fts JOIN messages m ON m.id = messages_fts.rowid JOIN chats c ON c.id = m.chat_id
                 WHERE messages_fts MATCH ?1 AND (?2 IS NULL OR c.service = ?2) AND m.deleted = 0
                 ORDER BY m.ts DESC LIMIT ?3 OFFSET ?4",
            )
            .map_err(err)?;
        let rows = st
            .query_map(params![fts, service, limit, offset], |r| {
                Ok(SearchHit {
                    message_id: r.get(0)?,
                    chat_id: r.get(1)?,
                    chat_title: r.get(2)?,
                    service: r.get(3)?,
                    account: r.get(4)?,
                    sender: r.get(5)?,
                    ts: r.get(6)?,
                    snippet: r.get(7)?,
                })
            })
            .map_err(err)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn media_list(&self, limit: i64, offset: i64) -> Result<Vec<MediaRow>, String> {
        let mut st = self
            .conn
            .prepare(&format!(
                "{MEDIA_SELECT} ORDER BY saved_at DESC, id DESC LIMIT ?1 OFFSET ?2"
            ))
            .map_err(err)?;
        let rows = st
            .query_map(params![limit, offset], media_row)
            .map_err(err)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn media_get(&self, id: i64) -> Result<Option<MediaRow>, String> {
        self.conn
            .query_row(
                &format!("{MEDIA_SELECT} WHERE id = ?1"),
                params![id],
                media_row,
            )
            .optional()
            .map_err(err)
    }

    /// Automatically saved media older than `cutoff` (ms). Hand downloads are never expired.
    pub fn expire_media(&self, cutoff: i64) -> Result<Vec<PathBuf>, String> {
        let mut st = self
            .conn
            .prepare("SELECT path FROM media WHERE saved_at < ?1 AND origin != 'download'")
            .map_err(err)?;
        let paths: Vec<PathBuf> = st
            .query_map(params![cutoff], |r| r.get::<_, String>(0))
            .map_err(err)?
            .filter_map(|r| r.ok())
            .map(PathBuf::from)
            .collect();
        self.conn
            .execute(
                "DELETE FROM media WHERE saved_at < ?1 AND origin != 'download'",
                params![cutoff],
            )
            .map_err(err)?;
        let mut unique = paths;
        unique.sort();
        unique.dedup();
        self.unreferenced(unique)
    }

    pub fn stats(&self) -> Result<Stats, String> {
        let count = |sql: &str| -> Result<i64, String> {
            self.conn.query_row(sql, [], |r| r.get(0)).map_err(err)
        };
        Ok(Stats {
            chats: count("SELECT count(*) FROM chats")?,
            messages: count("SELECT count(*) FROM messages")?,
            media: count("SELECT count(*) FROM media")?,
            db_bytes: std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0),
        })
    }

    /// Flushes the write-ahead log into the main file (before copying the archive).
    pub fn checkpoint(&self) -> Result<(), String> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(err)
    }
}

const CHAT_SELECT: &str = "SELECT c.id, c.service, c.account, c.chat_key, c.title, c.is_group, c.last_ts,
  (SELECT count(*) FROM messages m WHERE m.chat_id = c.id),
  (SELECT count(*) FROM messages m WHERE m.chat_id = c.id AND m.deleted = 0 AND coalesce(m.outgoing, 0) = 0 AND m.ord > c.last_read_ord)
  FROM chats c";

const MSG_SELECT: &str = "SELECT id, chat_id, msg_key, sender, outgoing, ts, ts_exact, time_text, ord, text, kind, source, deleted, edited FROM messages";

const MEDIA_SELECT: &str = "SELECT id, message_id, chat_id, kind, file_name, mime, size, path, encrypted, origin, saved_at FROM media";

fn chat_row(r: &rusqlite::Row) -> rusqlite::Result<ChatRow> {
    Ok(ChatRow {
        id: r.get(0)?,
        service: r.get(1)?,
        account: r.get(2)?,
        chat_key: r.get(3)?,
        title: r.get(4)?,
        is_group: r.get::<_, i64>(5)? != 0,
        last_ts: r.get(6)?,
        count: r.get(7)?,
        unseen: r.get(8)?,
    })
}

fn message_row(r: &rusqlite::Row) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: r.get(0)?,
        chat_id: r.get(1)?,
        key: r.get(2)?,
        sender: r.get(3)?,
        outgoing: r.get::<_, Option<i64>>(4)?.map(|v| v != 0),
        ts: r.get(5)?,
        ts_exact: r.get::<_, i64>(6)? != 0,
        time_text: r.get(7)?,
        ord: r.get(8)?,
        text: r.get(9)?,
        kind: r.get(10)?,
        source: r.get(11)?,
        deleted: r.get::<_, i64>(12)? != 0,
        edited: r.get::<_, i64>(13)? != 0,
        media: vec![],
    })
}

fn media_row(r: &rusqlite::Row) -> rusqlite::Result<MediaRow> {
    Ok(MediaRow {
        id: r.get(0)?,
        message_id: r.get(1)?,
        chat_id: r.get(2)?,
        kind: r.get(3)?,
        file_name: r.get(4)?,
        mime: r.get(5)?,
        size: r.get(6)?,
        path: r.get(7)?,
        encrypted: r.get::<_, i64>(8)? != 0,
        origin: r.get(9)?,
        saved_at: r.get(10)?,
    })
}

/// Turns what the person typed into a safe FTS5 query: every word must appear, as a prefix.
pub fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .take(12)
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(key: &str, text: &str) -> NewMessage {
        NewMessage {
            key: key.into(),
            text: text.into(),
            source: "live".into(),
            ..Default::default()
        }
    }

    fn open() -> (tempfile::TempDir, Archive) {
        let dir = tempfile::tempdir().unwrap();
        let a = Archive::open(dir.path(), &[1u8; 32]).unwrap();
        (dir, a)
    }

    #[test]
    fn wrong_key_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        {
            let a = Archive::open(dir.path(), &[1u8; 32]).unwrap();
            a.upsert_chat("telegram", "acc", "1", Some("x"), None)
                .unwrap();
        }
        assert!(Archive::open(dir.path(), &[2u8; 32]).is_err());
        assert!(Archive::open(dir.path(), &[1u8; 32]).is_ok());
    }

    #[test]
    fn dedup_edits_and_search_without_diacritics() {
        let (_d, a) = open();
        let c = a
            .upsert_chat("zalo", "acc", "k", Some("Gia đình"), Some(true))
            .unwrap();
        let (r, _, _) = a
            .write_batch(
                c,
                &[msg("1", "Tối nay ăn phở nhé"), msg("2", "ok")],
                1000,
                false,
            )
            .unwrap();
        assert_eq!(r[0].1, Outcome::Inserted);
        let (r, _, changed) = a
            .write_batch(c, &[msg("1", "Tối nay ăn phở nhé")], 2000, false)
            .unwrap();
        assert_eq!(r[0].1, Outcome::Unchanged);
        assert!(!changed);
        let (r, _, _) = a
            .write_batch(c, &[msg("1", "Tối nay ăn bún nhé")], 3000, false)
            .unwrap();
        assert_eq!(r[0].1, Outcome::Updated);
        let hits = a.search("bun", None, 10, 0).unwrap();
        assert_eq!(hits.len(), 1, "remove_diacritics finds bún from bun");
        assert!(
            a.search("pho", None, 10, 0).unwrap().is_empty(),
            "old text left the index"
        );
        let rows = a.messages(c, None, 10).unwrap();
        assert!(rows[0].edited);
    }

    #[test]
    fn neighbour_ordering_for_scrolled_history() {
        let (_d, a) = open();
        let c = a.upsert_chat("telegram", "acc", "k", None, None).unwrap();
        // First the newest messages are visible…
        let mut m3 = msg("3", "three");
        m3.next_key = Some("4".into());
        a.write_batch(c, &[msg("4", "four"), msg("5", "five")], 10_000, false)
            .unwrap();
        // …then the person scrolls up and older ones appear above.
        let mut m2 = msg("2", "two");
        m2.next_key = Some("3".into());
        a.write_batch(c, &[m2, m3], 50_000, false).unwrap();
        let keys: Vec<String> = a
            .messages(c, None, 10)
            .unwrap()
            .into_iter()
            .map(|m| m.key)
            .collect();
        assert_eq!(keys, vec!["2", "3", "4", "5"]);
    }

    #[test]
    fn unseen_and_recall() {
        let (_d, a) = open();
        let c = a.upsert_chat("whatsapp", "acc", "k", None, None).unwrap();
        let mut mine = msg("1", "hi");
        mine.outgoing = Some(true);
        a.write_batch(
            c,
            &[mine, msg("2", "hello"), msg("3", "are you there?")],
            1000,
            false,
        )
        .unwrap();
        let (unseen, _) = a.unseen_messages(c, 10).unwrap();
        assert_eq!(unseen.len(), 2);
        let mut recall = msg("3", "");
        recall.recalled = true;
        a.write_batch(c, &[recall], 2000, false).unwrap();
        let (unseen, _) = a.unseen_messages(c, 10).unwrap();
        assert_eq!(unseen.len(), 1, "recalled message is gone from summaries");
        a.mark_all_seen(c).unwrap();
        assert!(a.unseen_messages(c, 10).unwrap().0.is_empty());
    }

    #[test]
    fn fts_query_is_safe() {
        assert_eq!(fts_query("a \"b"), "\"a\"* \"\"\"b\"*");
        assert_eq!(fts_query("   "), "");
    }
}
