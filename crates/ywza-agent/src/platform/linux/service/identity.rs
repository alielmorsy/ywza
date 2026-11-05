//! The shared identity directory: ownership, migration from a per-user config.

use std::{fs, path::Path, process::Command};

use anyhow::bail;

use super::CONFIG_DIR;

/// Creates `CONFIG_DIR` (0700, root) and migrates `--user`'s identity once.
pub fn prepare(args: &[String]) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(CONFIG_DIR)?;
    fs::set_permissions(CONFIG_DIR, fs::Permissions::from_mode(0o700))?;
    chown_config(0, 0);
    // `--user` is only read to migrate that user's identity once.
    if let Some(user) = args
        .iter()
        .position(|a| a == "--user")
        .and_then(|i| args.get(i + 1))
    {
        if let Err(e) = migrate(user) {
            eprintln!("ywza-agent service: migrating {user}'s config: {e:#}");
        }
    }
    Ok(())
}

pub fn chown_config(uid: u32, gid: u32) {
    let _ = Command::new("chown")
        .args(["-R", &format!("{uid}:{gid}"), CONFIG_DIR])
        .status();
}

/// Copies `user`'s per-user config into `CONFIG_DIR` once, so it keeps its identity and pairings.
fn migrate(user: &str) -> anyhow::Result<()> {
    if Path::new(CONFIG_DIR).join("agent.key").exists() {
        return Ok(());
    }
    let (_, _, home) = passwd(user)?;
    let src = Path::new(&home).join(".config/ywza");
    let Ok(dir) = fs::read_dir(&src) else {
        return Ok(());
    };
    for e in dir.flatten() {
        if e.file_type()?.is_file() {
            fs::copy(e.path(), Path::new(CONFIG_DIR).join(e.file_name()))?;
        }
    }
    eprintln!(
        "ywza-agent service: copied {} to {CONFIG_DIR}",
        src.display()
    );
    Ok(())
}

/// Through NSS, so systemd dynamic users (newer gdm's `gdm-greeter`) resolve too.
pub fn passwd(user: &str) -> anyhow::Result<(u32, u32, String)> {
    let out = Command::new("getent").args(["passwd", user]).output()?;
    let line = String::from_utf8_lossy(&out.stdout);
    let f: Vec<&str> = line.trim().split(':').collect();
    if !out.status.success() || f.len() < 7 {
        bail!("no user {user}");
    }
    Ok((f[2].parse()?, f[3].parse()?, f[5].to_string()))
}
