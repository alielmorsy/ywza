//! Spots the screens an AI must not push through: codes, captchas, credential prompts, UAC.

use ywza_desktop::{Snapshot, WindowInfo};

const OTP: &[&str] = &[
    "verification code",
    "one-time",
    "one time code",
    "authenticator",
    "2-step",
    "two-factor",
    "security code",
];
const DIGIT_CODE: &[&str] = &["6-digit", "6 digit", "six-digit", "six digit"];
const CAPTCHA: &[&str] = &[
    "captcha",
    "i'm not a robot",
    "i\u{2019}m not a robot",
    "verify you are human",
    "verify you're human",
    "verify you\u{2019}re human",
];
const CREDENTIAL_WORDS: &[&str] = &["credential", "password", "user name", "username"];
const SIGN_IN: &[&str] = &["sign in", "sign-in", "log in", "log-in", "login"];

/// The blocker on `snap`'s window, as (kind, reason): otp, captcha, credentials or login.
/// Only the window's own element names and values count. A login wall is reported only when
/// the vault holds no secrets: with them the model can log in itself.
pub fn detect(snap: &Snapshot) -> Option<(&'static str, String)> {
    let items: Vec<(&str, String)> = snap
        .elements
        .iter()
        .map(|e| {
            let text = format!("{} {}", e.name, e.value.as_deref().unwrap_or_default());
            (e.kind, text.to_lowercase())
        })
        .collect();
    let (process, title) = (&snap.window.process, &snap.window.title);
    classify(process, title, &items, crate::vault::secrets().is_empty())
}

/// `items`: element kind and lowercase name + value.
fn classify(
    process: &str,
    title: &str,
    items: &[(&str, String)],
    allow_login: bool,
) -> Option<(&'static str, String)> {
    let any = |words: &[&str]| items.iter().any(|(_, t)| words.iter().any(|w| t.contains(w)));
    let title = title.to_lowercase();
    if process.eq_ignore_ascii_case("CredentialUIBroker.exe")
        || (title.contains("windows security") && any(CREDENTIAL_WORDS))
    {
        return Some(("credentials", "a Windows credential prompt needs the user".into()));
    }
    if any(CAPTCHA) {
        return Some(("captcha", "a captcha needs a human".into()));
    }
    let digit_code = items
        .iter()
        .any(|(_, t)| t.contains("code") && DIGIT_CODE.iter().any(|w| t.contains(w)));
    if any(OTP) || digit_code {
        return Some(("otp", "a one-time or verification code is needed".into()));
    }
    let password_field = items
        .iter()
        .any(|(k, t)| *k == "edit" && (t.contains("password") || t.contains("passcode")));
    if allow_login && password_field && any(SIGN_IN) {
        return Some(("login", "a login wall and no vault secrets to use".into()));
    }
    None
}

/// The UAC prompt, if one is up. Its secure desktop is invisible to UIA, so the consent.exe
/// window (its title when the process name is unreadable for a non-elevated agent) is the only
/// signal. shortcut: Windows only, Linux polkit prompts are not detected; add a polkit agent
/// window check when needed.
pub fn uac_window(windows: &[WindowInfo]) -> Option<&WindowInfo> {
    windows.iter().find(|w| {
        w.process.eq_ignore_ascii_case("consent.exe") || w.title == "User Account Control"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items<'a>(v: &[(&'a str, &str)]) -> Vec<(&'a str, String)> {
        v.iter().map(|(k, t)| (*k, t.to_lowercase())).collect()
    }

    fn kind(process: &str, title: &str, v: &[(&str, &str)], login: bool) -> Option<&'static str> {
        classify(process, title, &items(v), login).map(|r| r.0)
    }

    #[test]
    fn finds_each_blocker() {
        let k = |v: &[(&str, &str)]| kind("chrome.exe", "Page", v, true);
        assert_eq!(k(&[("edit", "Enter the 6-digit code")]), Some("otp"));
        assert_eq!(k(&[("text", "Open your Authenticator app")]), Some("otp"));
        assert_eq!(k(&[("text", "I'm not a robot")]), Some("captcha"));
        assert_eq!(
            k(&[("edit", "Password"), ("button", "Sign in")]),
            Some("login")
        );
        assert_eq!(
            kind("CredentialUIBroker.exe", "", &[], true),
            Some("credentials")
        );
        assert_eq!(
            kind("x.exe", "Windows Security", &[("edit", "Password")], true),
            Some("credentials")
        );
    }

    #[test]
    fn avoids_false_pauses() {
        let k = |v: &[(&str, &str)], login| kind("chrome.exe", "Page", v, login);
        assert_eq!(k(&[("button", "Save"), ("edit", "Name")], true), None);
        assert_eq!(k(&[("edit", "6 digit number")], true), None);
        assert_eq!(k(&[("edit", "Password"), ("button", "Sign in")], false), None);
        assert_eq!(k(&[("button", "Sign in")], true), None);
    }

    #[test]
    fn uac_by_process_or_title() {
        let w = |process: &str, title: &str| WindowInfo {
            id: 1,
            title: title.into(),
            process: process.into(),
            pid: 1,
            rect: ywza_desktop::Rect::default(),
            monitor: 0,
            minimized: false,
            hidden: false,
            focused: false,
            maximized: false,
        };
        assert!(uac_window(&[w("a.exe", "A"), w("Consent.exe", "")]).is_some());
        assert!(uac_window(&[w("?", "User Account Control")]).is_some());
        assert!(uac_window(&[w("a.exe", "A")]).is_none());
    }
}
