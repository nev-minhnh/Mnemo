//! Where each account's browser data lives, and sharing one browser process between accounts.
//!
//! Every WebView2 data folder starts its own browser: a GPU process, a network service,
//! storage, a manager… several hundred MB before any page loads. Accounts of *different*
//! apps can safely share one folder (their sign-ins live on different sites), so with
//! sharing on, accounts go into groups `shared-0`, `shared-1`… where a group holds at most
//! one account per app. Seven apps with one account each then run in a single browser.

use crate::settings::{Account, ServiceId, Settings};
use std::path::{Path, PathBuf};

/// The account's browser data folder.
pub fn dir(config_dir: &Path, account: &Account) -> PathBuf {
    let name = if account.profile.is_empty() {
        account.id.as_str()
    } else {
        account.profile.as_str()
    };
    config_dir.join("profiles").join(name)
}

/// The folder most enabled accounts run in (`shared-0` when there are none yet).
pub fn busiest(config_dir: &Path, settings: &Settings) -> PathBuf {
    let mut count: Vec<(&str, usize)> = Vec::new();
    for a in settings.accounts.iter().filter(|a| !a.disabled) {
        let name = folder_name(a);
        match count.iter_mut().find(|(n, _)| *n == name) {
            Some((_, c)) => *c += 1,
            None => count.push((name, 1)),
        }
    }
    // The first of the fullest ones, in sidebar order.
    let mut best: Option<(&str, usize)> = None;
    for c in count {
        if best.is_none_or(|b| c.1 > b.1) {
            best = Some(c);
        }
    }
    config_dir
        .join("profiles")
        .join(best.map_or("shared-0", |b| b.0))
}

/// A folder that couldn't be deleted now (Mnemo's own windows run in it) is deleted at the
/// next start instead.
pub fn delete_later(dir: &Path) {
    let _ = std::fs::write(dir.with_extension("delete"), b"");
}

/// The app whose sign-in this one can reuse: Instagram signs in with Facebook (Messenger),
/// Threads with Instagram.
pub fn partner(service: ServiceId) -> Option<ServiceId> {
    match service {
        ServiceId::Instagram => Some(ServiceId::Messenger),
        ServiceId::Threads => Some(ServiceId::Instagram),
        _ => None,
    }
}

/// Whether a folder already holds an account of this app (two can't share one folder).
fn holds(settings: &Settings, profile: &str, service: ServiceId, except: &str) -> bool {
    settings
        .accounts
        .iter()
        .any(|a| a.id != except && a.service == service && folder_name(a) == profile)
}

fn folder_name(a: &Account) -> &str {
    if a.profile.is_empty() {
        &a.id
    } else {
        &a.profile
    }
}

/// Puts the account into a shared group: the group of its partner app's account when that
/// has room (so Instagram can continue as the Facebook account), else the first group with
/// no other account of its app.
pub fn assign(settings: &mut Settings, id: &str) {
    let Some(i) = settings.accounts.iter().position(|a| a.id == id) else {
        return;
    };
    let service = settings.accounts[i].service;
    let preferred = partner(service).and_then(|p| {
        settings
            .accounts
            .iter()
            .filter(|a| a.service == p && a.profile.starts_with("shared-"))
            .map(|a| a.profile.clone())
            .find(|g| !holds(settings, g, service, id))
    });
    let group = preferred.unwrap_or_else(|| {
        let mut n = 0;
        while holds(settings, &format!("shared-{n}"), service, id) {
            n += 1;
        }
        format!("shared-{n}")
    });
    settings.accounts[i].profile = group;
}

/// Makes `id` use the same browser data as `with` (its sign-ins included). Refused when that
/// folder already holds an account of the same app.
pub fn link(settings: &mut Settings, id: &str, with: &str) -> Result<(), String> {
    let other = settings
        .account(with)
        .ok_or("Không tìm thấy tài khoản để dùng chung")?
        .clone();
    let me = settings
        .account(id)
        .ok_or("Không tìm thấy tài khoản")?
        .clone();
    let folder = folder_name(&other).to_string();
    if holds(settings, &folder, me.service, id) {
        return Err(format!(
            "{} đã có một tài khoản {} dùng chung đăng nhập này",
            other.label,
            me.service.display()
        ));
    }
    for a in settings.accounts.iter_mut() {
        if a.id == with || a.id == id {
            a.profile = folder.clone();
        }
    }
    Ok(())
}

/// Groups every account that still has its own folder (in sidebar order).
pub fn assign_all(settings: &mut Settings) {
    let ids: Vec<String> = settings
        .accounts
        .iter()
        .filter(|a| a.profile.is_empty())
        .map(|a| a.id.clone())
        .collect();
    for id in ids {
        assign(settings, &id);
    }
}

/// Turning sharing off: every account in a shared group goes back to a folder of its own,
/// made at next start as a copy of the group's (so it stays signed in).
pub fn unshare_all(settings: &mut Settings) {
    for a in settings.accounts.iter_mut() {
        if a.profile.starts_with("shared-") {
            a.pending_copy = std::mem::take(&mut a.profile);
        }
    }
}

/// Caches the copy can leave out (rebuilt by the browser).
const SKIP: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "GrShaderCache",
    "ShaderCache",
    "GraphiteDawnCache",
    "DawnCache",
    "DawnGraphiteCache",
    "Crashpad",
    "component_crx_cache",
];

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        if SKIP.iter().any(|s| name == *s) {
            continue;
        }
        let ty = e.file_type()?;
        let target = to.join(&name);
        if ty.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else if ty.is_file() {
            // A lock file of a browser that is gone may refuse to be read; skip it.
            let _ = std::fs::copy(e.path(), &target);
        }
    }
    Ok(())
}

/// At start, before any page opens:
/// - a group folder that doesn't exist yet is made from the own folder of its first
///   account, so that account stays signed in (the group's others sign in once more; their
///   old folders are left in place);
/// - an account leaving a group gets a copy of the group's folder, so nobody signs in again.
pub fn migrate(config_dir: &Path, settings: &mut Settings) {
    // Folders left to delete last time, unless an account moved in since.
    if let Ok(dir) = std::fs::read_dir(config_dir.join("profiles")) {
        for e in dir.flatten() {
            let path = e.path();
            if path.extension().is_some_and(|x| x == "delete") {
                let folder = path.with_extension("");
                let name = folder.file_name().unwrap_or_default().to_string_lossy();
                if !settings.accounts.iter().any(|a| folder_name(a) == name) {
                    let _ = std::fs::remove_dir_all(&folder);
                }
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    for a in settings.accounts.iter_mut() {
        if a.pending_copy.is_empty() {
            continue;
        }
        let from = config_dir.join("profiles").join(&a.pending_copy);
        let own = config_dir.join("profiles").join(&a.id);
        if from.exists() {
            if own.exists() {
                let _ = std::fs::remove_dir_all(&own);
            }
            match copy_dir(&from, &own) {
                Ok(()) => log::info!("{} has its own browser data again", a.label),
                Err(e) => log::warn!("cannot copy {} to {}: {e}", from.display(), own.display()),
            }
        }
        a.pending_copy.clear();
    }
    // Groups nobody uses any more.
    if let Ok(dir) = std::fs::read_dir(config_dir.join("profiles")) {
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("shared-") && !settings.accounts.iter().any(|a| a.profile == name) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    for a in &settings.accounts {
        if a.profile.is_empty() {
            continue;
        }
        let group = dir(config_dir, a);
        let own = config_dir.join("profiles").join(&a.id);
        if !group.exists() && own.exists() {
            match std::fs::rename(&own, &group) {
                Ok(()) => log::info!("{} now holds {}'s sign-in", group.display(), a.label),
                Err(e) => log::warn!("cannot move {} to {}: {e}", own.display(), group.display()),
            }
        }
    }
}

/// Whether another account still uses this account's folder.
pub fn shared_with_others(settings: &Settings, account: &Account) -> bool {
    !account.profile.is_empty()
        && settings
            .accounts
            .iter()
            .any(|a| a.id != account.id && a.profile == account.profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ServiceId;

    fn acc(id: &str, s: ServiceId) -> Account {
        Account {
            id: id.into(),
            service: s,
            ..Account::default()
        }
    }

    #[test]
    fn one_account_per_app_in_a_group() {
        let mut st = Settings::default();
        st.accounts = vec![
            acc("a", ServiceId::Zalo),
            acc("b", ServiceId::Whatsapp),
            acc("c", ServiceId::Zalo),
            acc("d", ServiceId::Telegram),
        ];
        assign_all(&mut st);
        let p: Vec<&str> = st.accounts.iter().map(|a| a.profile.as_str()).collect();
        assert_eq!(p, ["shared-0", "shared-0", "shared-1", "shared-0"]);
        st.accounts.push(acc("e", ServiceId::Zalo));
        assign(&mut st, "e");
        assert_eq!(st.accounts[4].profile, "shared-2");
        assert!(shared_with_others(&st, &st.accounts[0]));
        assert!(!shared_with_others(&st, &st.accounts[4]));
    }

    #[test]
    fn instagram_joins_messenger_and_threads_joins_instagram() {
        let mut st = Settings::default();
        st.accounts = vec![
            acc("m", ServiceId::Messenger),
            acc("w", ServiceId::Whatsapp),
            acc("i", ServiceId::Instagram),
        ];
        // Without sharing: an explicit link reuses the Messenger folder.
        link(&mut st, "i", "m").unwrap();
        assert_eq!(st.accounts[0].profile, "m");
        assert_eq!(st.accounts[2].profile, "m");
        st.accounts.push(acc("t", ServiceId::Threads));
        link(&mut st, "t", "i").unwrap();
        assert_eq!(st.accounts[3].profile, "m");
        st.accounts.push(acc("i2", ServiceId::Instagram));
        assert!(
            link(&mut st, "i2", "m").is_err(),
            "one Instagram per folder"
        );
        // With sharing: a second Messenger account's group is preferred by the next Instagram.
        let mut st = Settings::default();
        st.accounts = vec![
            acc("m1", ServiceId::Messenger),
            acc("i1", ServiceId::Instagram),
            acc("m2", ServiceId::Messenger),
            acc("i2", ServiceId::Instagram),
        ];
        assign_all(&mut st);
        let p: Vec<&str> = st.accounts.iter().map(|a| a.profile.as_str()).collect();
        assert_eq!(p, ["shared-0", "shared-0", "shared-1", "shared-1"]);
    }

    #[test]
    fn first_account_keeps_its_sign_in() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path();
        std::fs::create_dir_all(cfg.join("profiles/a/EBWebView")).unwrap();
        std::fs::create_dir_all(cfg.join("profiles/b/EBWebView")).unwrap();
        let mut st = Settings::default();
        st.accounts = vec![acc("a", ServiceId::Zalo), acc("b", ServiceId::Whatsapp)];
        assign_all(&mut st);
        migrate(cfg, &mut st);
        assert!(cfg.join("profiles/shared-0/EBWebView").exists());
        assert!(!cfg.join("profiles/a").exists());
        assert!(
            cfg.join("profiles/b").exists(),
            "the second account's old folder is kept"
        );
        // Sharing off: both get a copy of the group (sign-ins kept), the group goes away.
        std::fs::write(cfg.join("profiles/shared-0/EBWebView/Cookies"), b"c").unwrap();
        std::fs::create_dir_all(cfg.join("profiles/shared-0/EBWebView/Default/Cache")).unwrap();
        unshare_all(&mut st);
        migrate(cfg, &mut st);
        for id in ["a", "b"] {
            assert!(
                cfg.join(format!("profiles/{id}/EBWebView/Cookies"))
                    .exists()
            );
            assert!(
                !cfg.join(format!("profiles/{id}/EBWebView/Default/Cache"))
                    .exists()
            );
        }
        assert!(!cfg.join("profiles/shared-0").exists());
        assert!(
            st.accounts
                .iter()
                .all(|a| a.profile.is_empty() && a.pending_copy.is_empty())
        );
    }
}
