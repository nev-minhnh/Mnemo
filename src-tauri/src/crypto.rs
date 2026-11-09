//! Keys and encryption.
//!
//! * The archive key (32 random bytes) unlocks the SQLCipher database. On Windows it is
//!   stored wrapped with DPAPI (tied to the Windows user), in the app config folder, not
//!   next to the archive: copying the archive folder alone doesn't give its contents away.
//!   The person can show a recovery key to open the archive on another computer.
//! * Media saved with "encrypt media" on use AES-256-GCM with a key derived from it.
//! * API keys live in `secrets.bin`, a DPAPI-wrapped JSON map.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type Key = [u8; 32];

const MEDIA_MAGIC: &[u8; 8] = b"MNEMOv1\0";

#[cfg(windows)]
mod dpapi {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };

    fn take(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
        // SAFETY: DPAPI returned `cbData` valid bytes at `pbData`, which we free right after.
        let out = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
        unsafe {
            let _ = LocalFree(Some(HLOCAL(blob.pbData as _)));
        }
        out
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        // SAFETY: input points to `data` for the duration of the call; output is freed in `take`.
        unsafe {
            CryptProtectData(
                &input,
                windows::core::w!("Mnemo"),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(|e| format!("DPAPI protect failed: {e}"))?;
        }
        Ok(take(&output))
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        // SAFETY: as above.
        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(|e| format!("DPAPI unprotect failed: {e}"))?;
        }
        Ok(take(&output))
    }
}

#[cfg(not(windows))]
mod dpapi {
    // Development builds on other systems keep the bytes as they are (the file is 0600).
    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }
}

fn write_private(path: &Path, data: &[u8]) -> Result<(), String> {
    crate::util::write_atomic(path, data)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn key_path(config_dir: &Path) -> PathBuf {
    config_dir.join("keys").join("archive.key")
}

/// Loads the archive key, creating one the first time.
pub fn load_or_create_key(config_dir: &Path) -> Result<(Key, bool), String> {
    let path = key_path(config_dir);
    if path.exists() {
        let wrapped = std::fs::read(&path).map_err(|e| format!("cannot read archive key: {e}"))?;
        let raw = dpapi::unprotect(&wrapped)?;
        let key: Key = raw
            .as_slice()
            .try_into()
            .map_err(|_| "archive key file is damaged".to_string())?;
        return Ok((key, false));
    }
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    save_key(config_dir, &key)?;
    Ok((key, true))
}

pub fn save_key(config_dir: &Path, key: &Key) -> Result<(), String> {
    write_private(&key_path(config_dir), &dpapi::protect(key)?)
}

/// Recovery key shown to the person: the key in hex, grouped by 8 for readability.
pub fn recovery_string(key: &Key) -> String {
    hex::encode_upper(key)
        .as_bytes()
        .chunks(8)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn parse_recovery(input: &str) -> Result<Key, String> {
    let cleaned: String = input.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let bytes = hex::decode(&cleaned).map_err(|_| "Khóa khôi phục không hợp lệ".to_string())?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Khóa khôi phục phải có 64 ký tự hex".to_string())
}

fn media_cipher(master: &Key) -> Aes256Gcm {
    let hk = Hkdf::<Sha256>::new(Some(b"mnemo-media"), master);
    let mut okm = [0u8; 32];
    // 32 bytes is always a valid HKDF-SHA256 output length.
    let _ = hk.expand(b"aes-256-gcm v1", &mut okm);
    Aes256Gcm::new_from_slice(&okm).expect("32-byte key")
}

pub fn encrypt_media(master: &Key, plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = media_cipher(master);
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(&Nonce::from(nonce), plain)
        .map_err(|_| "encryption failed".to_string())?;
    let mut out = Vec::with_capacity(8 + 12 + ct.len());
    out.extend_from_slice(MEDIA_MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn is_encrypted_media(data: &[u8]) -> bool {
    data.len() > 20 && &data[..8] == MEDIA_MAGIC
}

pub fn decrypt_media(master: &Key, data: &[u8]) -> Result<Vec<u8>, String> {
    if !is_encrypted_media(data) {
        return Err("not an encrypted Mnemo file".into());
    }
    let nonce: [u8; 12] = data[8..20]
        .try_into()
        .map_err(|_| "damaged file".to_string())?;
    media_cipher(master)
        .decrypt(&Nonce::from(nonce), &data[20..])
        .map_err(|_| "the file can't be decrypted with this archive key".to_string())
}

/// API keys, wrapped with DPAPI. Values never leave the backend except in requests
/// to the provider they belong to.
pub struct Secrets {
    path: PathBuf,
    values: BTreeMap<String, String>,
}

impl Secrets {
    pub fn load(config_dir: &Path) -> Secrets {
        let path = config_dir.join("keys").join("secrets.bin");
        let values = std::fs::read(&path)
            .ok()
            .and_then(|w| dpapi::unprotect(&w).ok())
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        Secrets { path, values }
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.values
            .get(id)
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
    }

    pub fn has(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    pub fn set(&mut self, id: &str, value: Option<String>) -> Result<(), String> {
        match value
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            Some(v) => {
                self.values.insert(id.to_string(), v);
            }
            None => {
                self.values.remove(id);
            }
        }
        let raw = serde_json::to_vec(&self.values).map_err(|e| e.to_string())?;
        write_private(&self.path, &dpapi::protect(&raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_roundtrip_and_tamper() {
        let key = [7u8; 32];
        let enc = encrypt_media(&key, b"hello photo").unwrap();
        assert!(is_encrypted_media(&enc));
        assert_eq!(decrypt_media(&key, &enc).unwrap(), b"hello photo");
        let mut bad = enc.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(decrypt_media(&key, &bad).is_err());
        assert!(decrypt_media(&[8u8; 32], &enc).is_err());
    }

    #[test]
    fn recovery_roundtrip() {
        let key = [0xABu8; 32];
        let s = recovery_string(&key);
        assert_eq!(parse_recovery(&s).unwrap(), key);
        assert!(parse_recovery("abc").is_err());
    }

    #[test]
    fn key_and_secrets_persist() {
        let dir = tempfile::tempdir().unwrap();
        let (k1, created) = load_or_create_key(dir.path()).unwrap();
        assert!(created);
        let (k2, created) = load_or_create_key(dir.path()).unwrap();
        assert!(!created);
        assert_eq!(k1, k2);
        let mut s = Secrets::load(dir.path());
        s.set("openai", Some(" sk-test ".into())).unwrap();
        assert_eq!(Secrets::load(dir.path()).get("openai"), Some("sk-test"));
        s.set("openai", None).unwrap();
        assert!(!Secrets::load(dir.path()).has("openai"));
    }
}
