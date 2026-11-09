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

/// At start, before any page opens: a group folder that doesn't exist yet is made from the
/// own folder of its first account, so that account stays signed in. The group's other
/// accounts sign in once more; their old folders are left in place (and can be deleted).
pub fn migrate(config_dir: &Path, settings: &Settings) {
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
        migrate(cfg, &st);
        assert!(cfg.join("profiles/shared-0/EBWebView").exists());
        assert!(!cfg.join("profiles/a").exists());
        assert!(
            cfg.join("profiles/b").exists(),
            "the second account's old folder is kept"
        );
    }
}
