//! Where saved media goes on disk, and how it's written.
//!
//! Layout under the downloads folder the person chose:
//! `<Service>\<account>\<chat title> [id]\<YYYY-MM>\<file>` for automatically saved media,
//! `<Service>\<account>\Downloads\<file>` for files downloaded by hand.

use crate::crypto::{self, Key};
use crate::settings::ServiceId;
use crate::util::{safe_component, sha256_hex, short_hash, unique_path};
use std::path::{Path, PathBuf};

pub fn account_dir(downloads: &Path, service: ServiceId, account_label: &str) -> PathBuf {
    downloads
        .join(service.display())
        .join(safe_component(account_label, 60))
}

pub fn chat_dir(
    downloads: &Path,
    service: ServiceId,
    account_label: &str,
    chat_title: &str,
    chat_key: &str,
    ts_ms: i64,
) -> PathBuf {
    let title = if chat_title.trim().is_empty() {
        "Chat"
    } else {
        chat_title
    };
    let folder = format!("{} [{}]", safe_component(title, 60), short_hash(chat_key));
    let month = chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|d| d.format("%Y-%m").to_string())
        .unwrap_or_else(|| "unknown".into());
    account_dir(downloads, service, account_label)
        .join(safe_component(&folder, 80))
        .join(month)
}

pub fn manual_downloads_dir(downloads: &Path, service: ServiceId, account_label: &str) -> PathBuf {
    account_dir(downloads, service, account_label).join("Downloads")
}

pub struct Written {
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
    pub encrypted: bool,
}

/// Writes `data` as a new file in `dir` (never overwriting), encrypted when asked.
pub fn write_new(
    dir: &Path,
    file_name: &str,
    data: &[u8],
    encrypt: Option<&Key>,
) -> Result<Written, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Không tạo được thư mục {}: {e}", dir.display()))?;
    let sha = sha256_hex(data);
    let mut name = safe_component(file_name, 120);
    let bytes;
    let to_write: &[u8] = match encrypt {
        Some(key) => {
            name.push_str(".mnemo");
            bytes = crypto::encrypt_media(key, data)?;
            &bytes
        }
        None => data,
    };
    let path = unique_path(dir, &name);
    crate::util::write_atomic(&path, to_write)
        .map_err(|e| format!("Không ghi được {}: {e}", path.display()))?;
    Ok(Written {
        path,
        sha256: sha,
        size: data.len() as u64,
        encrypted: encrypt.is_some(),
    })
}

/// Reads a saved file, decrypting it when needed.
pub fn read_plain(path: &Path, key: &Key) -> Result<Vec<u8>, String> {
    let data =
        std::fs::read(path).map_err(|e| format!("Không đọc được {}: {e}", path.display()))?;
    if crypto::is_encrypted_media(&data) {
        crypto::decrypt_media(key, &data)
    } else {
        Ok(data)
    }
}

/// A file name for media that arrived without one.
pub fn fallback_name(kind: &str, mime: &str, ts_ms: i64) -> String {
    let when = chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|d| d.format("%Y%m%d-%H%M%S").to_string())
        .unwrap_or_else(|| "file".into());
    format!(
        "{kind}-{when}-{}.{}",
        &crate::util::random_id(2),
        crate::rules::ext_for_mime(mime)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_safe_and_grouped() {
        let d = chat_dir(
            Path::new("D:/x"),
            ServiceId::Zalo,
            "Cá nhân",
            "Nhóm: lớp 12/A",
            "123",
            1_762_659_000_000,
        );
        let s = d.to_string_lossy().replace('\\', "/");
        assert!(s.starts_with("D:/x/Zalo/Cá nhân/Nhóm_ lớp 12_A ["), "{s}");
        assert!(s.ends_with("/2025-11"), "{s}");
    }

    #[test]
    fn write_plain_and_encrypted() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_new(dir.path(), "../a.jpg", b"img", None).unwrap();
        assert_eq!(a.path.parent().unwrap(), dir.path());
        let key = [3u8; 32];
        let b = write_new(dir.path(), "a.jpg", b"img", Some(&key)).unwrap();
        assert!(b.path.to_string_lossy().ends_with(".jpg.mnemo"));
        assert_eq!(read_plain(&b.path, &key).unwrap(), b"img");
        assert_eq!(read_plain(&a.path, &key).unwrap(), b"img");
        assert_eq!(a.sha256, b.sha256);
    }
}
