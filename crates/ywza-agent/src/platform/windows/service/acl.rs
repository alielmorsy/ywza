//! Access to the shared identity directory, through the OS's own tools.

use std::{fs, os::windows::process::CommandExt, path::Path, path::PathBuf, process::Command};

use anyhow::{bail, Context};

const NO_WINDOW: u32 = 0x0800_0000;

/// Full control for SYSTEM and Administrators, nothing inherited from ProgramData (whose
/// Users read access would expose the keys).
const LOCKED: &str = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";

pub fn config_dir() -> PathBuf {
    std::env::var_os("ProgramData")
        .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from)
        .join("ywza")
}

/// Back to [`LOCKED`] in one step, dropping any user grant (also one a crashed service left),
/// then every file inherits it and is owned by SYSTEM again.
pub fn lock(dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    let lit = dir.display().to_string().replace('\'', "''");
    run(Command::new("powershell").args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &format!(
            "$a = Get-Acl -LiteralPath '{lit}'; $a.SetSecurityDescriptorSddlForm('{LOCKED}', 'Access'); Set-Acl -LiteralPath '{lit}' -AclObject $a"
        ),
    ]))?;
    if fs::read_dir(dir)?.next().is_some() {
        icacls(&dir.join("*"), &["/reset", "/T", "/C", "/Q"])?;
    }
    icacls(dir, &["/setowner", "*S-1-5-18", "/T", "/C", "/Q"])
}

/// Lets `user` (`DOMAIN\name`) read and write the identity while their agent runs. `None` is
/// SYSTEM, which already has it.
pub fn grant(dir: &Path, user: Option<&str>) -> anyhow::Result<()> {
    match user {
        Some(u) => icacls(dir, &["/grant", &format!("{u}:(OI)(CI)M"), "/Q"]),
        None => Ok(()),
    }
}

fn icacls(path: &Path, args: &[&str]) -> anyhow::Result<()> {
    run(Command::new("icacls").arg(path).args(args))
}

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    let out = cmd
        .creation_flags(NO_WINDOW)
        .output()
        .with_context(|| format!("running {:?}", cmd.get_program()))?;
    if !out.status.success() {
        bail!(
            "{:?} failed: {}{}",
            cmd.get_program(),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}
