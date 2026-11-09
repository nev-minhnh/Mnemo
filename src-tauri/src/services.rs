//! The five apps: where they live, which pages they may open, and the page settings
//! (selectors, CSS) the injected script uses. Page settings ship as JSON and can be
//! overridden per service in `<config>\services\<id>.json` when an app changes its page.

use crate::settings::ServiceId;
use serde_json::{Map, Value};
use std::path::Path;
use url::Url;

pub struct ServiceDef {
    pub id: ServiceId,
    pub url: &'static str,
    /// Hosts (and their subdomains) the page may navigate to.
    pub hosts: &'static [&'static str],
    /// When set, only these paths stay in the app ("/" means exactly the root);
    /// anything else opens in the browser.
    pub paths: Option<&'static [&'static str]>,
    /// Hosts media may be downloaded from by URL (signed CDN links).
    pub media_hosts: &'static [&'static str],
    /// Other sites a sign-in may pass through (Instagram's "continue with Facebook").
    pub partner_hosts: &'static [&'static str],
    pub default_config: &'static str,
}

pub const SERVICES: [ServiceDef; 9] = [
    ServiceDef {
        id: ServiceId::Whatsapp,
        url: "https://web.whatsapp.com/",
        hosts: &["web.whatsapp.com"],
        paths: None,
        media_hosts: &["whatsapp.net"],
        partner_hosts: &[],
        default_config: include_str!("../inject/services/whatsapp.json"),
    },
    ServiceDef {
        id: ServiceId::Messenger,
        url: "https://www.facebook.com/messages/",
        hosts: &["facebook.com"],
        paths: Some(&[
            "/",
            "/messages",
            "/login",
            "/login.php",
            "/checkpoint",
            "/two_step_verification",
            "/two_factor",
            "/auth_platform",
            "/recover",
            "/cookie",
            "/privacy/consent",
            "/dialog",
            "/home.php",
            "/confirmemail.php",
            "/ajax",
            "/security",
        ]),
        media_hosts: &["fbcdn.net", "fbsbx.com"],
        partner_hosts: &["instagram.com"],
        default_config: include_str!("../inject/services/messenger.json"),
    },
    ServiceDef {
        id: ServiceId::Instagram,
        url: "https://www.instagram.com/direct/inbox/",
        hosts: &["instagram.com"],
        paths: Some(&[
            "/",
            "/direct",
            "/accounts",
            "/challenge",
            "/two_factor",
            "/auth_platform",
            "/api",
        ]),
        media_hosts: &["cdninstagram.com", "fbcdn.net"],
        partner_hosts: &["facebook.com"],
        default_config: include_str!("../inject/services/instagram.json"),
    },
    ServiceDef {
        id: ServiceId::Telegram,
        url: "https://web.telegram.org/a/",
        hosts: &["web.telegram.org"],
        paths: None,
        media_hosts: &[],
        partner_hosts: &[],
        default_config: include_str!("../inject/services/telegram.json"),
    },
    ServiceDef {
        id: ServiceId::Zalo,
        url: "https://chat.zalo.me/",
        hosts: &["chat.zalo.me", "id.zalo.me", "zalo.me", "zaloapp.com"],
        paths: None,
        media_hosts: &["zadn.vn", "zdn.vn", "zalo.me", "zaloapp.com"],
        partner_hosts: &[],
        default_config: include_str!("../inject/services/zalo.json"),
    },
    ServiceDef {
        id: ServiceId::Discord,
        url: "https://discord.com/app",
        hosts: &["discord.com", "discordapp.com"],
        paths: Some(&[
            "/",
            "/app",
            "/channels",
            "/login",
            "/register",
            "/oauth2",
            "/ra",
            "/verify",
            "/reset",
            "/authorize",
            "/api",
        ]),
        media_hosts: &["discordapp.com", "discordapp.net", "discord.com"],
        partner_hosts: &[],
        default_config: include_str!("../inject/services/discord.json"),
    },
    ServiceDef {
        id: ServiceId::Wechat,
        url: "https://web.wechat.com/",
        hosts: &[
            "web.wechat.com",
            "wechat.com",
            "wx.qq.com",
            "wx2.qq.com",
            "qq.com",
        ],
        paths: None,
        media_hosts: &["qq.com", "qpic.cn", "wechat.com"],
        partner_hosts: &[],
        default_config: include_str!("../inject/services/wechat.json"),
    },
    ServiceDef {
        id: ServiceId::Threads,
        url: "https://www.threads.com/messages",
        hosts: &["threads.com", "threads.net"],
        paths: None,
        media_hosts: &["cdninstagram.com", "fbcdn.net"],
        // Threads signs in with an Instagram account.
        partner_hosts: &["instagram.com", "facebook.com"],
        default_config: include_str!("../inject/services/threads.json"),
    },
    ServiceDef {
        id: ServiceId::Slack,
        url: "https://app.slack.com/client",
        hosts: &["slack.com"],
        paths: None,
        media_hosts: &["slack.com", "slack-edge.com"],
        partner_hosts: &[],
        default_config: include_str!("../inject/services/slack.json"),
    },
];

pub fn def(id: ServiceId) -> &'static ServiceDef {
    SERVICES
        .iter()
        .find(|s| s.id == id)
        .expect("every service has a definition")
}

pub fn host_matches(host: &str, allowed: &[&str]) -> bool {
    let host = host.to_ascii_lowercase();
    allowed
        .iter()
        .any(|a| host == *a || host.ends_with(&format!(".{a}")))
}

/// What to do with a navigation of the service's page.
#[derive(Debug, PartialEq, Eq)]
pub enum Nav {
    Allow,
    OpenExternally,
    Block,
}

pub fn navigation(def: &ServiceDef, url: &Url) -> Nav {
    match url.scheme() {
        "about" | "blob" | "data" => return Nav::Allow,
        "http" | "https" => {}
        "mailto" | "tel" => return Nav::OpenExternally,
        _ => return Nav::Block,
    }
    let Some(host) = url.host_str() else {
        return Nav::Block;
    };
    if !host_matches(host, def.hosts) {
        return Nav::OpenExternally;
    }
    if url.scheme() != "https" {
        return Nav::Block;
    }
    match def.paths {
        None => Nav::Allow,
        Some(paths) => {
            let path = url.path();
            let ok = paths.iter().any(|p| {
                if *p == "/" {
                    path == "/"
                } else {
                    path == *p
                        || path.starts_with(&format!("{p}/"))
                        || path.starts_with(&format!("{p}?"))
                }
            });
            if ok { Nav::Allow } else { Nav::OpenExternally }
        }
    }
}

/// Sign-in and identity-check pages of the big identity providers. Facebook and Instagram
/// sometimes ask you to confirm it is you with your Google account; that page has to open
/// inside the account's own profile, or the confirmation never reaches the app.
const SIGN_IN_HOSTS: &[&str] = &[
    "accounts.google.com",
    "accounts.youtube.com",
    "appleid.apple.com",
    "idmsa.apple.com",
    "login.microsoftonline.com",
    "login.live.com",
    "account.live.com",
    "login.yahoo.com",
];

/// A page of an identity provider's sign-in flow.
pub fn is_sign_in(url: &Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    if host_matches(&host, SIGN_IN_HOSTS) || host.starts_with("accounts.google.") {
        return true;
    }
    // Google's sign-in also passes through www.google.<country>/accounts, /signin, /sorry.
    let google =
        host == "google.com" || host.starts_with("www.google.") || host.starts_with("google.");
    google
        && [
            "/accounts",
            "/signin",
            "/sorry",
            "/recaptcha",
            "/ServiceLogin",
        ]
        .iter()
        .any(|p| url.path().starts_with(p))
}

/// How long after an identity-provider page the service may use any of its own pages
/// (and its partner's), so the provider can hand the result back wherever it needs to.
pub const SIGN_IN_GRACE_MS: u64 = 15 * 60 * 1000;

/// `navigation`, plus sign-in flows: identity-provider pages stay in the account's
/// webview, and for a while afterwards the service's own pages are not limited by path.
/// `sign_in_until` is the per-webview end of that window (ms since the epoch).
pub fn navigation_signin(
    def: &ServiceDef,
    url: &Url,
    sign_in_until: &std::sync::atomic::AtomicU64,
    now_ms: u64,
) -> Nav {
    use std::sync::atomic::Ordering;
    if is_sign_in(url) {
        sign_in_until.store(now_ms + SIGN_IN_GRACE_MS, Ordering::Relaxed);
        return Nav::Allow;
    }
    let nav = navigation(def, url);
    if nav == Nav::OpenExternally
        && url.scheme() == "https"
        && now_ms < sign_in_until.load(Ordering::Relaxed)
        && url
            .host_str()
            .is_some_and(|h| host_matches(h, def.hosts) || host_matches(h, def.partner_hosts))
    {
        return Nav::Allow;
    }
    nav
}

/// Page settings for a service: the shipped JSON, with the person's override on top.
pub fn page_config(config_dir: &Path, id: ServiceId) -> Value {
    let mut base: Value =
        serde_json::from_str(def(id).default_config).unwrap_or(Value::Object(Map::new()));
    let dir = config_dir.join("services");
    let _ = std::fs::create_dir_all(&dir);
    // A reference copy of the defaults, refreshed every start.
    let _ = std::fs::write(
        dir.join(format!("{}.default.json", id.as_str())),
        def(id).default_config,
    );
    let override_path = dir.join(format!("{}.json", id.as_str()));
    if let Ok(text) = std::fs::read_to_string(&override_path) {
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(over)) => {
                if let Value::Object(b) = &mut base {
                    for (k, v) in over {
                        b.insert(k, v);
                    }
                }
            }
            Ok(_) => log::warn!(
                "{} must contain a JSON object; ignored",
                override_path.display()
            ),
            Err(e) => log::warn!(
                "{} is not valid JSON ({e}); ignored",
                override_path.display()
            ),
        }
    }
    base
}

/// The script injected into every page of an account's webview.
pub fn init_script(service: ServiceId, account_id: &str, config: &Value) -> String {
    let boot = serde_json::json!({
        "service": service.as_str(),
        "account": account_id,
        "cfg": config,
    });
    format!(
        "(function(){{ if (window.top !== window) return; window.__MNEMO__ = {boot}; }})();\n{}",
        include_str!("../inject/capture.js")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn messenger_stays_in_messages() {
        let d = def(ServiceId::Messenger);
        assert_eq!(
            navigation(d, &u("https://www.facebook.com/messages/t/123")),
            Nav::Allow
        );
        assert_eq!(navigation(d, &u("https://www.facebook.com/")), Nav::Allow);
        assert_eq!(
            navigation(d, &u("https://www.facebook.com/login/?next=x")),
            Nav::Allow
        );
        assert_eq!(
            navigation(d, &u("https://www.facebook.com/someone.profile")),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation(d, &u("https://www.facebook.com/messagesx")),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation(d, &u("https://evil.com/messages")),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation(d, &u("https://evilfacebook.com/messages")),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation(d, &u("http://www.facebook.com/messages")),
            Nav::Block
        );
        assert_eq!(navigation(d, &u("file:///C:/x")), Nav::Block);
    }

    #[test]
    fn google_check_stays_inside() {
        use std::sync::atomic::AtomicU64;
        let d = def(ServiceId::Messenger);
        let until = AtomicU64::new(0);
        let now = 1_000_000;
        let fb_other = u("https://www.facebook.com/some/verify/return?x=1");
        assert_eq!(
            navigation_signin(d, &fb_other, &until, now),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation_signin(
                d,
                &u("https://accounts.google.com/o/oauth2/v2/auth?client_id=x"),
                &until,
                now
            ),
            Nav::Allow
        );
        assert_eq!(
            navigation_signin(
                d,
                &u("https://accounts.google.com.vn/accounts/SetSID"),
                &until,
                now
            ),
            Nav::Allow
        );
        assert_eq!(
            navigation_signin(d, &fb_other, &until, now + 1000),
            Nav::Allow
        );
        assert_eq!(
            navigation_signin(d, &u("https://www.google.com/search?q=x"), &until, now),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation_signin(d, &u("https://evil.com/"), &until, now),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation_signin(d, &fb_other, &until, now + SIGN_IN_GRACE_MS + 1),
            Nav::OpenExternally
        );
        assert_eq!(
            navigation_signin(d, &u("http://accounts.google.com/"), &until, now),
            Nav::OpenExternally
        );
        let ig = def(ServiceId::Instagram);
        let until = AtomicU64::new(0);
        navigation_signin(ig, &u("https://accounts.google.com/x"), &until, now);
        assert_eq!(
            navigation_signin(ig, &u("https://www.facebook.com/checkpoint/x"), &until, now),
            Nav::Allow
        );
    }

    #[test]
    fn configs_parse() {
        for s in SERVICES.iter() {
            let v: Value = serde_json::from_str(s.default_config)
                .unwrap_or_else(|e| panic!("{:?}: {e}", s.id));
            assert!(v.get("message").is_some(), "{:?}", s.id);
        }
    }
}
