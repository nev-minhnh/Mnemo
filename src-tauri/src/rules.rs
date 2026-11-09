//! Decides whether a piece of media is saved automatically.

use crate::settings::DownloadRules;
use serde::Serialize;

/// Hard cap for anything saved automatically, whatever the rules say.
pub const HARD_CAP_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Image,
    Video,
    Audio,
    Document,
    Other,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Image => "image",
            Category::Video => "video",
            Category::Audio => "audio",
            Category::Document => "document",
            Category::Other => "other",
        }
    }
}

const IMAGE: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "bmp", "avif", "tif", "tiff", "svg",
];
const VIDEO: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "m4v", "3gp", "wmv", "mpeg", "mpg",
];
const AUDIO: &[&str] = &[
    "mp3", "m4a", "ogg", "oga", "opus", "wav", "aac", "flac", "amr", "weba", "wma",
];
const DOCUMENT: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "rtf", "odt", "ods", "odp", "csv",
    "zip", "rar", "7z", "epub", "md", "json", "xml", "html", "htm", "tex",
];

/// Category from an extension, falling back to a MIME type when there is no extension.
pub fn categorize(ext: &str, mime: &str) -> Category {
    let ext = ext.to_ascii_lowercase();
    if IMAGE.contains(&ext.as_str()) {
        return Category::Image;
    }
    if VIDEO.contains(&ext.as_str()) {
        return Category::Video;
    }
    if AUDIO.contains(&ext.as_str()) {
        return Category::Audio;
    }
    if DOCUMENT.contains(&ext.as_str()) {
        return Category::Document;
    }
    let mime = mime.to_ascii_lowercase();
    if ext.is_empty() {
        if mime.starts_with("image/") {
            return Category::Image;
        }
        if mime.starts_with("video/") {
            return Category::Video;
        }
        if mime.starts_with("audio/") {
            return Category::Audio;
        }
        if mime == "application/pdf" || mime.starts_with("text/") {
            return Category::Document;
        }
    }
    Category::Other
}

/// The usual extension for a MIME type, for media that arrives without a file name.
pub fn ext_for_mime(mime: &str) -> &'static str {
    match mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/heic" => "heic",
        "image/avif" => "avif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "audio/ogg" => "ogg",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/webm" => "weba",
        "audio/wav" | "audio/x-wav" => "wav",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

pub struct Candidate<'a> {
    pub ext: &'a str,
    pub mime: &'a str,
    /// Size when known (0 = not known yet; checked again once downloaded).
    pub size: u64,
    pub is_group: bool,
    pub chat_key: &'a str,
    pub chat_title: &'a str,
}

/// `Ok(())` when the media should be saved; otherwise a short reason for the log/report.
pub fn decide(rules: &DownloadRules, c: &Candidate) -> Result<(), &'static str> {
    if !rules.enabled {
        return Err("tự động tải đang tắt");
    }
    let ext = c.ext.to_ascii_lowercase();
    if rules
        .block_extensions
        .iter()
        .any(|b| b.trim_start_matches('.').eq_ignore_ascii_case(&ext))
    {
        return Err("định dạng bị chặn");
    }
    if c.is_group && !rules.include_groups {
        return Err("không tải trong nhóm");
    }
    let excluded = rules
        .excluded_chats
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .any(|x| {
            x.eq_ignore_ascii_case(c.chat_key)
                || x.to_lowercase() == c.chat_title.trim().to_lowercase()
        });
    if excluded {
        return Err("hội thoại bị loại trừ");
    }
    let explicitly_allowed = !ext.is_empty()
        && rules
            .allow_extensions
            .iter()
            .any(|a| a.trim_start_matches('.').eq_ignore_ascii_case(&ext));
    if !explicitly_allowed {
        let ok = match categorize(&ext, c.mime) {
            Category::Image => rules.images,
            Category::Video => rules.videos,
            Category::Audio => rules.audio,
            Category::Document => rules.documents,
            Category::Other => rules.other,
        };
        if !ok {
            return Err("loại tệp không được chọn");
        }
    }
    if c.size > max_bytes(rules) {
        return Err("tệp vượt dung lượng tối đa");
    }
    Ok(())
}

/// Largest size allowed by the rules, never above the hard cap.
pub fn max_bytes(rules: &DownloadRules) -> u64 {
    if rules.max_size_mb == 0 {
        HARD_CAP_BYTES
    } else {
        (rules.max_size_mb.saturating_mul(1024 * 1024)).min(HARD_CAP_BYTES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand<'a>(ext: &'a str, size: u64) -> Candidate<'a> {
        Candidate {
            ext,
            mime: "",
            size,
            is_group: false,
            chat_key: "123",
            chat_title: "Mẹ",
        }
    }

    #[test]
    fn default_rules() {
        let r = DownloadRules::default();
        assert!(decide(&r, &cand("jpg", 1000)).is_ok());
        assert!(decide(&r, &cand("pdf", 1000)).is_ok());
        assert!(
            decide(&r, &cand("mp4", 1000)).is_err(),
            "videos are off by default"
        );
        assert!(decide(&r, &cand("exe", 10)).is_err());
        assert!(decide(&r, &cand("EXE", 10)).is_err());
        assert!(decide(&r, &cand("jpg", 26 * 1024 * 1024)).is_err());
        assert!(decide(&r, &cand("dwg", 10)).is_err(), "other is off");
    }

    #[test]
    fn allow_list_exclusions_and_groups() {
        let mut r = DownloadRules::default();
        r.allow_extensions = vec![".dwg".into(), "exe".into()];
        assert!(decide(&r, &cand("dwg", 10)).is_ok());
        assert!(
            decide(&r, &cand("exe", 10)).is_err(),
            "block list wins over allow list"
        );
        r.excluded_chats = vec!["mẹ".into()];
        assert!(decide(&r, &cand("jpg", 10)).is_err());
        r.excluded_chats.clear();
        r.include_groups = false;
        let mut c = cand("jpg", 10);
        c.is_group = true;
        assert!(decide(&r, &c).is_err());
        r.enabled = false;
        assert!(decide(&r, &cand("jpg", 10)).is_err());
    }

    #[test]
    fn mime_fallback() {
        assert_eq!(categorize("", "image/png"), Category::Image);
        assert_eq!(categorize("", "application/octet-stream"), Category::Other);
        assert_eq!(ext_for_mime("audio/ogg; codecs=opus"), "ogg");
    }
}
