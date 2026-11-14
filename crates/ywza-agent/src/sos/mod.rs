//! Agent SOS: when the AI is stuck or hits something only a human can pass, the run pauses, the
//! user takes over, and the run continues with a digest of what they did.

mod ask;
mod detect;
mod digest;
mod teach;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::oneshot;
use ywza_desktop::{Snapshot, WindowInfo};
use ywza_proto::SessionId;

use crate::plans::PlanStep;

pub use detect::{detect, uac_window};

/// Local tool results in a row that changed nothing before the run asks for help.
pub const NO_CHANGE_STREAK: u32 = 3;

/// What the user did while the run waited.
struct Handback {
    note: String,
    steps: Vec<PlanStep>,
}

struct Paused {
    tx: oneshot::Sender<Handback>,
    /// The window's latest known state, for mapping the user's taps to elements.
    snap: Option<Snapshot>,
    steps: Vec<PlanStep>,
}

/// A blocker the loop must act on.
pub(crate) enum Blocker {
    /// A saved fix exists: tell the model instead of pausing.
    Hint(String),
    Pause(Pause),
}

pub(crate) struct Pause {
    kind: &'static str,
    reason: String,
    window: Option<WindowInfo>,
    /// The window's snapshot when the blocker showed up.
    before: Option<Snapshot>,
}

#[derive(Default)]
pub struct Sos {
    paused: Arc<Mutex<HashMap<SessionId, Paused>>>,
    /// Sightings per session and blocker (kind and window), so one blocker raises once.
    seen: Mutex<HashMap<SessionId, HashMap<String, u8>>>,
}

impl Sos {
    /// Hands the run back to the user's note. False: the session is not waiting for help.
    pub fn resume(&self, sid: SessionId, note: String) -> bool {
        let Some(p) = self.paused.lock().unwrap().remove(&sid) else {
            return false;
        };
        p.tx.send(Handback { note, steps: p.steps }).is_ok()
    }

    /// Forgets a finished session.
    pub fn forget(&self, sid: SessionId) {
        self.paused.lock().unwrap().remove(&sid);
        self.seen.lock().unwrap().remove(&sid);
    }

    /// Counts a sighting of `sig` in the session; returns how many times it was seen.
    fn sighting(&self, sid: SessionId, sig: String) -> u8 {
        let mut seen = self.seen.lock().unwrap();
        let n = seen.entry(sid).or_default().entry(sig).or_default();
        *n = n.saturating_add(1);
        *n
    }
}

/// `fix-<kind>-<process without extension>`: the plan the user's taps are saved as.
fn fix_name(kind: &str, process: &str) -> String {
    let stem = process.rsplit_once('.').map_or(process, |(s, _)| s);
    let stem: String = stem
        .chars()
        .map(|c| match c {
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => c.to_ascii_lowercase(),
            _ => '-',
        })
        .take(40)
        .collect();
    let stem = stem.trim_matches('-');
    format!("fix-{kind}-{}", if stem.is_empty() { "unknown" } else { stem })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_fix_plans() {
        assert_eq!(fix_name("otp", "chrome.exe"), "fix-otp-chrome");
        assert_eq!(fix_name("uac", "?"), "fix-uac-unknown");
        assert_eq!(fix_name("login", "My App.exe"), "fix-login-my-app");
        assert!(crate::plans::store::valid_name(&fix_name("otp", "a b/c.exe")));
    }

    #[test]
    fn resume_needs_a_pause() {
        let s = Sos::default();
        let sid = SessionId::new();
        assert!(!s.resume(sid, String::new()));
        assert_eq!(s.sighting(sid, "otp:1".into()), 1);
        assert_eq!(s.sighting(sid, "otp:1".into()), 2);
        assert_eq!(s.sighting(sid, "otp:2".into()), 1);
        s.forget(sid);
        assert_eq!(s.sighting(sid, "otp:1".into()), 1);
    }
}
