//! Persistent settings (`settings.json` in the app config folder).
//!
//! Secrets (API keys) are never stored here: see `crypto::Secrets`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ServiceId {
    Whatsapp,
    Messenger,
    Instagram,
    Telegram,
    Zalo,
    Discord,
    Wechat,
    Threads,
    Slack,
    Linkedin,
}

impl ServiceId {
    pub const ALL: [ServiceId; 10] = [
        ServiceId::Whatsapp,
        ServiceId::Messenger,
        ServiceId::Instagram,
        ServiceId::Telegram,
        ServiceId::Zalo,
        ServiceId::Discord,
        ServiceId::Wechat,
        ServiceId::Threads,
        ServiceId::Slack,
        ServiceId::Linkedin,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceId::Whatsapp => "whatsapp",
            ServiceId::Messenger => "messenger",
            ServiceId::Instagram => "instagram",
            ServiceId::Telegram => "telegram",
            ServiceId::Zalo => "zalo",
            ServiceId::Discord => "discord",
            ServiceId::Wechat => "wechat",
            ServiceId::Threads => "threads",
            ServiceId::Slack => "slack",
            ServiceId::Linkedin => "linkedin",
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            ServiceId::Whatsapp => "WhatsApp",
            ServiceId::Messenger => "Messenger",
            ServiceId::Instagram => "Instagram",
            ServiceId::Telegram => "Telegram",
            ServiceId::Zalo => "Zalo",
            ServiceId::Discord => "Discord",
            ServiceId::Wechat => "WeChat",
            ServiceId::Threads => "Threads",
            ServiceId::Slack => "Slack",
            ServiceId::Linkedin => "LinkedIn",
        }
    }

    pub fn parse(s: &str) -> Option<ServiceId> {
        ServiceId::ALL.into_iter().find(|x| x.as_str() == s)
    }
}

/// Which media gets saved automatically (live capture and imports).
/// Files the person downloads by hand are always saved.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DownloadRules {
    pub enabled: bool,
    pub images: bool,
    pub videos: bool,
    pub audio: bool,
    pub documents: bool,
    pub other: bool,
    /// Extensions always allowed, whatever their category (e.g. "dwg").
    pub allow_extensions: Vec<String>,
    /// Extensions never saved automatically (executables by default).
    pub block_extensions: Vec<String>,
    /// Largest file saved automatically, in MB. 0 = no limit (a hard cap still applies).
    pub max_size_mb: u64,
    pub include_groups: bool,
    /// Chats (titles or ids) whose media is never saved automatically.
    pub excluded_chats: Vec<String>,
    /// Delete automatically saved media older than this many days. 0 = keep forever.
    pub retention_days: u32,
}

impl Default for DownloadRules {
    fn default() -> Self {
        DownloadRules {
            enabled: true,
            images: true,
            videos: false,
            audio: true,
            documents: true,
            other: false,
            allow_extensions: vec![],
            block_extensions: [
                "exe", "msi", "bat", "cmd", "com", "ps1", "scr", "js", "jse", "vbs", "vbe", "jar",
                "lnk", "hta", "cpl", "msc", "reg", "pif",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            max_size_mb: 25,
            include_groups: true,
            excluded_chats: vec![],
            retention_days: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    pub id: String,
    pub service: ServiceId,
    pub label: String,
    /// The person's own display name in this service; used to mark imported messages as theirs.
    pub self_name: String,
    /// Save messages from this account into the archive.
    pub capture: bool,
    /// Per-account rules; `None` uses the global rules.
    pub rules: Option<DownloadRules>,
    /// Show Mnemo's notifications for this account.
    pub notify: bool,
    /// Browser data folder under `profiles\`: empty = the account's own (`<id>`), or a
    /// shared group (`shared-0`, `shared-1`…) whose accounts run in one browser process.
    #[serde(default)]
    pub profile: String,
    /// Turned off for now: no page, no memory, no notifications; the sign-in stays on disk.
    #[serde(default)]
    pub disabled: bool,
    /// Set when sharing is turned off: at next start this account gets its own copy of
    /// that shared folder (sign-ins included), then this is cleared.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pending_copy: String,
}

impl Default for Account {
    fn default() -> Self {
        Account {
            id: String::new(),
            service: ServiceId::Telegram,
            label: String::new(),
            self_name: String::new(),
            capture: true,
            rules: None,
            notify: true,
            profile: String::new(),
            disabled: false,
            pending_copy: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Claude Code CLI, signed in with the person's own Claude account.
    ClaudeCode,
    /// Codex CLI, signed in with the person's ChatGPT account.
    CodexCli,
    /// OpenAI API key.
    Openai,
    /// Anthropic API key.
    Anthropic,
    /// Google AI Studio (Gemini) API key, through Google's OpenAI-compatible endpoint.
    Gemini,
    /// xAI (Grok) API key.
    Xai,
    /// Any other OpenAI-compatible endpoint: DeepSeek, MiniMax, OpenRouter, llama.cpp…
    Compatible,
}

impl ProviderKind {
    pub fn uses_key(&self) -> bool {
        !matches!(self, ProviderKind::ClaudeCode | ProviderKind::CodexCli)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    /// API base URL (OpenAI-style providers) — ignored by the CLIs.
    pub base_url: String,
    /// Known models (fetched from the provider or typed by the person).
    pub models: Vec<String>,
    pub enabled: bool,
    /// Optional full path to the CLI program (claude / codex) when it isn't on PATH.
    pub cli_path: String,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            id: String::new(),
            kind: ProviderKind::Compatible,
            name: String::new(),
            base_url: String::new(),
            models: vec![],
            enabled: true,
            cli_path: String::new(),
        }
    }
}

/// A provider + model + effort + thinking choice.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiChoice {
    pub provider: String,
    /// Empty = the provider's default model.
    pub model: String,
    /// "", "low", "medium", "high" (and "xhigh"/"max" for Claude Code).
    pub effort: String,
    pub thinking: bool,
}

impl Default for AiChoice {
    fn default() -> Self {
        AiChoice {
            provider: "claude-code".into(),
            model: String::new(),
            effort: "low".into(),
            thinking: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub builtin: bool,
}

impl Default for Skill {
    fn default() -> Self {
        Skill {
            id: String::new(),
            name: String::new(),
            instruction: String::new(),
            builtin: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub providers: Vec<ProviderConfig>,
    /// Defaults for the writing popup.
    pub writing: AiChoice,
    /// Defaults for the chat panel (summaries, reply suggestions, questions).
    pub chat: AiChoice,
    /// Check spelling/grammar automatically when typing pauses (costs tokens).
    pub auto_check: bool,
    pub skills: Vec<Skill>,
    /// Most messages sent to the AI as context for one request.
    pub max_context_messages: u32,
    /// Extra standing instructions for every request (tone, language…).
    pub personal_instructions: String,
    /// What the messages are for: the writing assistant checks drafts against these.
    pub goals: WritingGoals,
    /// Words the writing assistant never flags (names, slang, brand words…).
    pub dictionary: Vec<String>,
}

/// Writing goals for chat drafts (as Grammarly's goals, made for messages).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WritingGoals {
    /// "casual", "neutral" or "formal".
    pub formality: String,
    /// "general", "friends", "family", "colleagues", "customers", "boss".
    pub audience: String,
    /// "chat", "inform", "request", "persuade", "apologise", "thank".
    pub intent: String,
    /// "auto" (the draft's own), "vi", "en-US", "en-GB".
    pub variant: String,
}

impl Default for WritingGoals {
    fn default() -> Self {
        WritingGoals {
            formality: "neutral".into(),
            audience: "general".into(),
            intent: "chat".into(),
            variant: "auto".into(),
        }
    }
}

impl Default for AiSettings {
    fn default() -> Self {
        AiSettings {
            providers: default_providers(),
            writing: AiChoice::default(),
            chat: AiChoice {
                effort: "medium".into(),
                ..AiChoice::default()
            },
            auto_check: false,
            skills: default_skills(),
            max_context_messages: 400,
            personal_instructions: String::new(),
            goals: WritingGoals::default(),
            dictionary: vec![],
        }
    }
}

pub fn default_providers() -> Vec<ProviderConfig> {
    let p = |id: &str, kind, name: &str, base: &str, models: &[&str]| ProviderConfig {
        id: id.into(),
        kind,
        name: name.into(),
        base_url: base.into(),
        models: models.iter().map(|s| s.to_string()).collect(),
        enabled: true,
        cli_path: String::new(),
    };
    vec![
        p(
            "claude-code",
            ProviderKind::ClaudeCode,
            "Claude Code (tài khoản Claude)",
            "",
            &["opus", "sonnet", "haiku"],
        ),
        p(
            "codex",
            ProviderKind::CodexCli,
            "ChatGPT (Codex CLI)",
            "",
            &[],
        ),
        p(
            "openai",
            ProviderKind::Openai,
            "OpenAI API",
            "https://api.openai.com/v1",
            &[],
        ),
        p(
            "gemini",
            ProviderKind::Gemini,
            "Gemini (Google AI Studio)",
            "https://generativelanguage.googleapis.com/v1beta/openai",
            &[],
        ),
        p(
            "xai",
            ProviderKind::Xai,
            "Grok (xAI)",
            "https://api.x.ai/v1",
            &[],
        ),
        p(
            "anthropic",
            ProviderKind::Anthropic,
            "Anthropic API",
            "https://api.anthropic.com",
            &[],
        ),
    ]
}

pub fn default_skills() -> Vec<Skill> {
    let s = |id: &str, name: &str, instruction: &str| Skill {
        id: id.into(),
        name: name.into(),
        instruction: instruction.into(),
        builtin: true,
    };
    vec![
        s(
            "polite",
            "Lịch sự hơn",
            "Rewrite the text to be more polite and considerate, keeping the meaning and the language.",
        ),
        s(
            "friendly",
            "Thân thiện hơn",
            "Rewrite the text to sound warmer and more friendly, keeping the meaning and the language.",
        ),
        s(
            "concise",
            "Ngắn gọn hơn",
            "Rewrite the text to be shorter and to the point, keeping every important detail and the language.",
        ),
        s(
            "clear",
            "Rõ ràng hơn",
            "Rewrite the text so it is clearer and easier to understand, keeping the meaning and the language.",
        ),
        s(
            "formal",
            "Trang trọng (công việc)",
            "Rewrite the text in a professional, formal register suitable for work, keeping the meaning and the language.",
        ),
        s(
            "to-en",
            "Dịch sang tiếng Anh",
            "Translate the text into natural English. Keep names, numbers and links unchanged.",
        ),
        s(
            "to-vi",
            "Dịch sang tiếng Việt",
            "Translate the text into natural Vietnamese. Keep names, numbers and links unchanged.",
        ),
    ]
}

/// Mnemo's own notifications for new messages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotifySettings {
    pub enabled: bool,
    /// Show the message text (off = "New message" only).
    pub show_content: bool,
    /// Seconds a notification stays on screen.
    pub duration_secs: u32,
    /// Total unread count on Mnemo's taskbar icon.
    pub taskbar_badge: bool,
}

impl Default for NotifySettings {
    fn default() -> Self {
        NotifySettings {
            enabled: true,
            show_content: true,
            duration_secs: 8,
            taskbar_badge: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// Folder holding the encrypted message archive (`mnemo.db`).
    pub archive_dir: PathBuf,
    /// Folder holding saved media and downloads.
    pub downloads_dir: PathBuf,
    /// Master switch for saving messages from the open apps.
    pub capture_enabled: bool,
    /// Encrypt automatically saved media on disk (opened through Mnemo).
    pub encrypt_media: bool,
    /// Keep the archived copy of a message after its sender recalls/deletes it.
    pub keep_recalled: bool,
    pub rules: DownloadRules,
    pub accounts: Vec<Account>,
    /// "system", "light" or "dark".
    pub theme: String,
    /// Interface language: "vi" or "en". AI answers follow it.
    pub language: String,
    /// Sidebar shows only avatars.
    pub sidebar_compact: bool,
    /// Old switch for a right-hand sidebar; read once into `sidebar_pos`.
    #[serde(skip_serializing)]
    pub sidebar_right: bool,
    /// Where Mnemo's bar sits: "left", "right", "top" or "bottom".
    pub sidebar_pos: String,
    /// Start hidden in the tray when Windows starts.
    pub start_with_windows: bool,
    /// The window's close button hides Mnemo in the tray instead of quitting.
    pub close_to_tray: bool,
    /// Started with Windows: minimised on the taskbar (icon and unread count showing)
    /// instead of only in the tray.
    pub start_minimized: bool,
    /// New accounts join a shared browser process (one per group) instead of starting their own.
    pub share_processes: bool,
    /// Turn off the browser's GPU acceleration (saves memory; calls and video less smooth).
    pub disable_gpu: bool,
    pub notify: NotifySettings,
    pub ai: AiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            archive_dir: PathBuf::new(),
            downloads_dir: PathBuf::new(),
            capture_enabled: true,
            encrypt_media: false,
            keep_recalled: false,
            rules: DownloadRules::default(),
            accounts: vec![],
            theme: "system".into(),
            language: "vi".into(),
            sidebar_compact: false,
            sidebar_right: false,
            sidebar_pos: "top".into(),
            start_with_windows: false,
            close_to_tray: true,
            start_minimized: false,
            share_processes: true,
            disable_gpu: false,
            notify: NotifySettings::default(),
            ai: AiSettings::default(),
        }
    }
}

impl Settings {
    pub fn path(config_dir: &Path) -> PathBuf {
        config_dir.join("settings.json")
    }

    /// Loads settings, filling folders that were never chosen with defaults under `documents`.
    /// A damaged file is kept aside (`settings.json.broken-<id>`) instead of being overwritten.
    pub fn load(config_dir: &Path, documents: &Path) -> Settings {
        let path = Self::path(config_dir);
        let mut s = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    log::error!(
                        "settings.json is unreadable ({e}); keeping a copy and using defaults"
                    );
                    let _ = std::fs::rename(
                        &path,
                        path.with_extension(format!("json.broken-{}", crate::util::random_id(3))),
                    );
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        };
        // Settings from before top/bottom existed only knew left/right.
        s.sidebar_pos = match s.sidebar_pos.as_str() {
            "left" | "right" | "top" | "bottom" => s.sidebar_pos.clone(),
            _ if s.sidebar_right => "right".into(),
            _ => "top".into(),
        };
        if s.archive_dir.as_os_str().is_empty() {
            s.archive_dir = documents.join("Mnemo").join("Archive");
        }
        if s.downloads_dir.as_os_str().is_empty() {
            s.downloads_dir = documents.join("Mnemo").join("Downloads");
        }
        // Providers added in newer versions appear for people upgrading.
        for p in default_providers() {
            if !s.ai.providers.iter().any(|x| x.id == p.id) {
                s.ai.providers.push(p);
            }
        }
        s.version = SETTINGS_VERSION;
        s
    }

    pub fn save(&self, config_dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        crate::util::write_atomic(&Self::path(config_dir), &bytes)
    }

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Rules that apply to an account (its own, or the global ones).
    pub fn rules_for(&self, account_id: &str) -> DownloadRules {
        self.account(account_id)
            .and_then(|a| a.rules.clone())
            .unwrap_or_else(|| self.rules.clone())
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.ai.providers.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_fills_defaults_and_keeps_broken_file() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs");
        std::fs::write(Settings::path(dir.path()), b"{not json").unwrap();
        let s = Settings::load(dir.path(), &docs);
        assert_eq!(s.archive_dir, docs.join("Mnemo").join("Archive"));
        assert!(s.ai.providers.iter().any(|p| p.id == "claude-code"));
        let broken = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains("broken"));
        assert!(broken);
    }

    #[test]
    fn roundtrip_and_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(Settings::path(dir.path()), br#"{"capture_enabled": false}"#).unwrap();
        let s = Settings::load(dir.path(), dir.path());
        assert!(!s.capture_enabled);
        assert!(s.rules.images);
        s.save(dir.path()).unwrap();
        let again = Settings::load(dir.path(), dir.path());
        assert!(!again.capture_enabled);
    }
}
