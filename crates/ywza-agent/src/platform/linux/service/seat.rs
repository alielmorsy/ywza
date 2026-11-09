//! What seat0 shows: the active logind session and its environment.

use std::{collections::HashMap, fs, path::Path, process::Command};

use anyhow::{bail, Context};

/// The session's user and environment.
pub struct Target {
    pub user: String,
    pub env: HashMap<String, String>,
}

fn loginctl(args: &[&str]) -> Option<String> {
    let out = Command::new("loginctl").args(args).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

pub fn active_session() -> Option<String> {
    loginctl(&["show-seat", "seat0", "-p", "ActiveSession", "--value"])
}

fn greeter_session() -> Option<String> {
    loginctl(&["show-seat", "seat0", "-p", "Sessions", "--value"])?
        .split_whitespace()
        .find(|id| {
            loginctl(&["show-session", id, "-p", "Class", "--value"]).as_deref() == Some("greeter")
        })
        .map(String::from)
}

pub fn activate_greeter() {
    if let Some(g) = greeter_session() {
        tracing::info!(greeter = %g, "service: no active session, activating the greeter");
        let _ = Command::new("loginctl").args(["activate", &g]).status();
    }
}

pub fn target(id: &str) -> anyhow::Result<Target> {
    let prop = |p: &str| loginctl(&["show-session", id, "-p", p, "--value"]).unwrap_or_default();
    if prop("Type") != "x11" {
        bail!("type {:?}, not x11", prop("Type"));
    }
    let class = prop("Class");
    if class != "greeter" && class != "user" {
        bail!("class {class:?}");
    }
    let env = session_env(id).context("no process with DISPLAY yet")?;
    Ok(Target {
        user: prop("Name"),
        env,
    })
}

fn read_env(pid: &Path) -> Option<HashMap<String, String>> {
    let raw = fs::read(pid.join("environ")).ok()?;
    Some(
        raw.split(|&b| b == 0)
            .filter_map(|kv| {
                let kv = String::from_utf8_lossy(kv);
                let (k, v) = kv.split_once('=')?;
                Some((k.to_string(), v.to_string()))
            })
            .collect(),
    )
}

/// Environment of a process in `session-<id>.scope` that has DISPLAY set.
fn session_env(id: &str) -> Option<HashMap<String, String>> {
    let scope = format!("/session-{id}.scope");
    for e in fs::read_dir("/proc").ok()?.flatten() {
        let p = e.path();
        let in_session = fs::read_to_string(p.join("cgroup"))
            .is_ok_and(|c| c.lines().any(|l| l.ends_with(&scope)));
        if !in_session {
            continue;
        }
        if let Some(env) = read_env(&p).filter(|env| env.contains_key("DISPLAY")) {
            return Some(env);
        }
    }
    None
}
