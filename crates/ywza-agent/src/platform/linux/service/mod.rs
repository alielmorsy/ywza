//! `ywza-agent service`: run as root (systemd, see `dist/ywza-agent.service`).
//!
//! Keeps one `ywza-agent run` on whatever X11 session seat0 shows: the login screen (logind
//! class `greeter`) or any user's session (class `user`). The agent runs as that session's user
//! with the session's environment, so a paired client can type the password at the greeter and
//! drive whoever is logged in. Needs an X11 greeter (gdm: `WaylandEnable=false`).
//!
//! All of them share one machine identity in `/var/lib/ywza` (`YWZA_CONFIG_DIR`). Root keeps it
//! and hands it to the served session's user only while that user's agent runs.
//!
//! Wayland/tty sessions get no agent. A locked session is still the same session, so the agent
//! there types into the lock screen like into any window.

mod identity;
mod process;
mod seat;
mod xauth;

use std::process::Child;
use std::time::Duration;

use identity::chown_config;

const CONFIG_DIR: &str = "/var/lib/ywza";

pub fn main(args: &[String]) -> anyhow::Result<()> {
    identity::prepare(args)?;
    let exe = std::env::current_exe()?;
    process::stop_stale(&exe);
    eprintln!("ywza-agent service: following seat0");

    let mut cur: Option<(String, Child)> = None;
    let mut last_note = String::new();
    let mut idle = 0;
    loop {
        let active = seat::active_session();
        // After some logouts the display manager leaves seat0 on an empty VT, which a remote
        // viewer can never leave on its own: bring the login screen back.
        idle = if active.is_some() { 0 } else { idle + 1 };
        if idle == 3 {
            seat::activate_greeter();
        }
        let want = active.and_then(|id| match seat::target(&id) {
            Ok(t) => Some((id, t)),
            Err(e) => {
                let note = format!("session {id}: {e:#}");
                if note != last_note {
                    eprintln!("ywza-agent service: {note}");
                    last_note = note;
                }
                None
            }
        });
        if let Some((id, child)) = &mut cur {
            let exited = !matches!(child.try_wait(), Ok(None));
            if exited || want.as_ref().map(|(w, _)| w) != Some(id) {
                process::stop(child);
                chown_config(0, 0);
                cur = None;
            }
        }
        if let (None, Some((id, t))) = (&cur, want) {
            match process::spawn(&exe, t) {
                Ok(c) => {
                    eprintln!("ywza-agent service: agent {} on session {id}", c.id());
                    last_note.clear();
                    cur = Some((id, c));
                }
                Err(e) => {
                    chown_config(0, 0);
                    eprintln!("ywza-agent service: start on session {id}: {e:#}");
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
