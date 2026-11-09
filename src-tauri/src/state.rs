//! Shared application state.

use crate::archive::Archive;
use crate::crypto::{Key, Secrets};
use crate::settings::Settings;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

#[derive(Clone, Debug, Default, Serialize)]
pub struct CaptureStats {
    pub messages: u64,
    pub media: u64,
    pub last_at: i64,
    pub last_error: Option<String>,
}

/// The conversation open in an account's page right now.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LiveContext {
    pub chat_key: Option<String>,
    pub title: Option<String>,
    pub is_group: bool,
}

/// What the writing popup is working on.
#[derive(Clone, Debug, Default)]
pub struct AssistState {
    pub account_id: String,
    pub text: String,
    /// Proofreading results by SHA-256 of the text (auto-check and the popup share them).
    pub cache: HashMap<String, serde_json::Value>,
}

pub struct AppState {
    pub config_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub archive: Mutex<Option<Archive>>,
    pub archive_error: Mutex<Option<String>>,
    pub key: Mutex<Option<Key>>,
    pub secrets: Mutex<Secrets>,
    pub unread: Mutex<HashMap<String, u32>>,
    pub stats: Mutex<HashMap<String, CaptureStats>>,
    pub context: Mutex<HashMap<String, LiveContext>>,
    pub visible: Mutex<Option<String>>,
    pub panel_open: Mutex<bool>,
    pub assist: Mutex<AssistState>,
    /// Width of the shell's sidebar (logical px); accounts are laid out to its right.
    pub sidebar_w: Mutex<f64>,
    /// Which window edge Mnemo's bar is on: left, right, top or bottom.
    pub sidebar_pos: Mutex<String>,
    /// Account avatars as data URLs (detected from the app's page or chosen by the person).
    pub avatars: Mutex<HashMap<String, String>>,
    /// Notifications on screen, newest first.
    pub notices: Mutex<Vec<crate::notify::Notice>>,
    /// Per account: names of the conversations the app's chat list marks unread.
    pub senders: Mutex<HashMap<String, Vec<String>>>,
    /// Accounts whose page reports its own unread count (instead of the title).
    pub page_unread: Mutex<std::collections::HashSet<String>>,
    /// Per account: logical px of the page's top kept above the window (hidden header).
    pub crop_top: Mutex<HashMap<String, f64>>,
    /// Per account: the draft check the page last reported (for the Writing assistant panel).
    pub writing: Mutex<HashMap<String, serde_json::Value>>,
    /// The apps' own icons (data URLs) by service id.
    pub icons: Mutex<HashMap<String, String>>,
    pub http: reqwest::Client,
}

/// Locks a mutex, recovering the data if another thread panicked while holding it.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn key(&self) -> Result<Key, String> {
        lock(&self.key).ok_or_else(|| "Khóa kho lưu trữ chưa sẵn sàng".to_string())
    }

    /// Runs `f` with the open archive, or explains why it isn't open.
    pub fn with_archive<T>(
        &self,
        f: impl FnOnce(&mut Archive) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = lock(&self.archive);
        match guard.as_mut() {
            Some(a) => f(a),
            None => Err(lock(&self.archive_error)
                .clone()
                .unwrap_or_else(|| "Kho lưu trữ chưa mở".into())),
        }
    }

    pub fn bump_stats(&self, account: &str, messages: u64, media: u64, error: Option<String>) {
        let mut s = lock(&self.stats);
        let e = s.entry(account.to_string()).or_default();
        e.messages += messages;
        e.media += media;
        e.last_at = crate::util::now_ms();
        if error.is_some() {
            e.last_error = error;
        }
    }

    /// Opens the archive in the configured folder with the current key.
    pub fn open_archive(&self) {
        let dir = lock(&self.settings).archive_dir.clone();
        let key = *lock(&self.key);
        let result = match key {
            Some(k) => Archive::open(&dir, &k),
            None => Err("Khóa kho lưu trữ chưa sẵn sàng".into()),
        };
        match result {
            Ok(a) => {
                *lock(&self.archive) = Some(a);
                *lock(&self.archive_error) = None;
            }
            Err(e) => {
                log::error!("archive: {e}");
                *lock(&self.archive) = None;
                *lock(&self.archive_error) = Some(e);
            }
        }
    }
}
