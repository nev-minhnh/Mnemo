//! Small helpers shared by every module: names, ids, hashing and time.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Current time in milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A random lowercase hex id of `bytes` random bytes.
pub fn random_id(bytes: usize) -> String {
    use rand::RngCore;
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

/// SHA-256 of `data` as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// Short stable hash (first 8 hex chars of SHA-256) used to keep folder names unique.
pub fn short_hash(s: &str) -> String {
    sha256_hex(s.as_bytes())[..8].to_string()
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Turns any text (a chat title, a file name sent by someone else) into a safe single
/// path component for Windows: no separators, no reserved names, no trailing dots or
/// spaces, bounded length. Never returns an empty string.
pub fn safe_component(input: &str, max_chars: usize) -> String {
    let mut out: String = input
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    out = out.trim().trim_end_matches(['.', ' ']).to_string();
    // Leading dots would make hidden files or "..".
    while out.starts_with('.') {
        out.remove(0);
    }
    if out.chars().count() > max_chars {
        // Keep the extension when shortening a file name.
        let (stem, ext) = split_ext(&out);
        let keep = max_chars.saturating_sub(ext.chars().count() + 1).max(1);
        let stem: String = stem.chars().take(keep).collect();
        out = if ext.is_empty() {
            stem
        } else {
            format!("{stem}.{ext}")
        };
        out = out.trim_end_matches(['.', ' ']).to_string();
    }
    let upper = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&upper.as_str()) {
        out = format!("_{out}");
    }
    if out.is_empty() { "_".to_string() } else { out }
}

/// Splits "name.ext" into ("name", "ext"); ext is lowercase-insensitive input untouched.
pub fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 && !name[i + 1..].contains(' ') => {
            (&name[..i], &name[i + 1..])
        }
        _ => (name, ""),
    }
}

/// Lowercase extension of a file name, without the dot.
pub fn ext_of(name: &str) -> String {
    split_ext(name).1.to_ascii_lowercase()
}

/// Returns `dir/name`, or `dir/name (2).ext`, `dir/name (3).ext`… whichever doesn't exist yet.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = split_ext(name);
    for i in 2..10_000 {
        let candidate = if ext.is_empty() {
            format!("{stem} ({i})")
        } else {
            format!("{stem} ({i}).{ext}")
        };
        let p = dir.join(candidate);
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{}-{}", random_id(4), name))
}

/// Writes a file through a temporary sibling and a rename, so a crash never leaves
/// a half-written file in place of a good one.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", random_id(4)));
    std::fs::write(&tmp, data)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Truncates `s` to at most `max` characters (not bytes).
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_component_handles_hostile_names() {
        assert_eq!(safe_component("a/b\\c:d", 50), "a_b_c_d");
        assert_eq!(safe_component("..\\..\\evil.exe", 50), "_.._evil.exe");
        assert_eq!(safe_component("CON", 50), "_CON");
        assert_eq!(safe_component("con.txt", 50), "_con.txt");
        assert_eq!(safe_component("   ", 50), "_");
        assert_eq!(safe_component("name. . ", 50), "name");
        assert_eq!(safe_component("Nhóm gia đình", 50), "Nhóm gia đình");
        let long = format!("{}.pdf", "x".repeat(300));
        let s = safe_component(&long, 40);
        assert!(s.chars().count() <= 40 && s.ends_with(".pdf"), "{s}");
    }

    #[test]
    fn unique_path_appends_counter() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = unique_path(dir.path(), "a.txt");
        std::fs::write(&p1, b"1").unwrap();
        let p2 = unique_path(dir.path(), "a.txt");
        assert_eq!(p2.file_name().unwrap(), "a (2).txt");
    }
}
