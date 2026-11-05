//! Starting, stopping and finding `ywza-agent run` processes.

use std::{
    fs,
    path::Path,
    process::{Child, Command},
    time::Duration,
};

use anyhow::Context;

use super::{
    identity::{chown_config, passwd},
    seat::Target,
    xauth::xserver_auth,
    CONFIG_DIR,
};

/// The X cookie handed to the agent: the original may be readable only by root (`-auth`).
const XAUTH_COPY: &str = "/run/ywza.xauth";

/// One identity, one agent: stop one left by a previous service run and wait for it; two
/// live endpoints with one key leave the new one unreachable.
pub fn stop_stale(exe: &Path) {
    let stale = stale_agents(exe);
    if stale.is_empty() {
        return;
    }
    let _ = Command::new("kill").arg("-INT").args(&stale).status();
    for _ in 0..50 {
        if stale.iter().all(|p| !Path::new("/proc").join(p).exists()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = Command::new("kill").arg("-KILL").args(&stale).status();
}

/// Pids of `ywza-agent run` processes left by a previous service run, including ones from
/// older versions that used per-user config dirs (`KillMode=process` keeps them alive).
fn stale_agents(exe: &Path) -> Vec<String> {
    let Ok(dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| {
            // A rebuilt binary leaves running copies pointing at "<path> (deleted)".
            fs::read_link(e.path().join("exe")).is_ok_and(|p| {
                let p = p.to_string_lossy();
                p.strip_suffix(" (deleted)").unwrap_or(&p) == exe.to_string_lossy()
            })
        })
        .filter(|e| {
            fs::read(e.path().join("cmdline"))
                .is_ok_and(|c| c.split(|&b| b == 0).nth(1) == Some(b"run"))
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

pub fn spawn(exe: &Path, t: Target) -> anyhow::Result<Child> {
    let Target { user, mut env } = t;
    let (uid, gid, home) = passwd(&user)?;
    let cookie = match env.get("XAUTHORITY") {
        Some(c) => Some(c.clone()),
        None => xserver_auth(&env["DISPLAY"]),
    };
    if let Some(cookie) = cookie {
        fs::copy(&cookie, XAUTH_COPY).with_context(|| format!("copying {cookie}"))?;
        std::os::unix::fs::chown(XAUTH_COPY, Some(uid), Some(gid))?;
        env.insert("XAUTHORITY".into(), XAUTH_COPY.into());
    }
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    env.entry("PATH".into()).or_insert(path);
    env.entry("XDG_SESSION_TYPE".into()).or_insert("x11".into());
    for (k, v) in [
        ("HOME", home.as_str()),
        ("USER", &user),
        ("LOGNAME", &user),
        ("YWZA_CONFIG_DIR", CONFIG_DIR),
    ] {
        env.insert(k.into(), v.into());
    }
    chown_config(uid, gid);
    // Greeter users often have no home directory.
    let cwd = if Path::new(&home).is_dir() {
        &home
    } else {
        "/"
    };
    use std::os::unix::process::CommandExt;
    Command::new(exe)
        .arg("run")
        .env_clear()
        .envs(env)
        .current_dir(cwd)
        .gid(gid)
        .uid(uid)
        .spawn()
        .context("spawning ywza-agent run")
}

/// SIGINT (the agent's graceful exit), then SIGKILL after 5 s.
pub fn stop(child: &mut Child) {
    let _ = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status();
    for _ in 0..50 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}
