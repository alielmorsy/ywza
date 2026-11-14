//! Shell integration: launch plans that make known interactive shells emit prompt marks and cwd.

mod scripts;

use std::fs;
use std::path::{Path, PathBuf};

pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, PartialEq, Eq)]
enum Shell {
    Pwsh,
    Bash,
    Zsh,
}

/// Only a bare shell invocation is interactive; anything with arguments runs unchanged.
fn detect(command: &str) -> Option<(String, Shell)> {
    let c = command.trim();
    let program = match c.strip_prefix('"') {
        Some(q) => {
            let (p, rest) = q.split_once('"')?;
            rest.trim().is_empty().then_some(p)?
        }
        None => (c.split_whitespace().count() == 1).then_some(c)?,
    };
    let stem = Path::new(program).file_stem()?.to_str()?.to_lowercase();
    let shell = match stem.as_str() {
        "pwsh" | "powershell" => Shell::Pwsh,
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        _ => return None,
    };
    Some((program.to_string(), shell))
}

/// `None` for unknown shells or when the integration files cannot be written.
pub fn plan(command: &str) -> Option<Launch> {
    let (program, shell) = detect(command)?;
    let dir = base_dir();
    let (args, env) = match shell {
        Shell::Pwsh => (
            vec!["-NoExit".into(), "-Command".into(), scripts::PWSH.into()],
            vec![],
        ),
        Shell::Bash => {
            let rc = put(&dir.join("bashrc.sh"), scripts::BASH)?;
            (vec!["--rcfile".into(), rc, "-i".into()], vec![])
        }
        Shell::Zsh => {
            let zdir = dir.join("zsh");
            put(&zdir.join(".zshenv"), scripts::ZSHENV)?;
            put(&zdir.join(".zshrc"), scripts::ZSHRC)?;
            let ours = slashed(&zdir);
            let user = std::env::var("ZDOTDIR")
                .or_else(|_| std::env::var("HOME"))
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_default();
            (
                vec![],
                vec![
                    ("ZDOTDIR".into(), ours.clone()),
                    ("YWZA_ZDOTDIR_OURS".into(), ours),
                    ("YWZA_ZDOTDIR_USER".into(), user),
                ],
            )
        }
    };
    Some(Launch { program, args, env })
}

fn slashed(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Written only when missing or different, so concurrent terminals share the files.
fn put(path: &Path, content: &str) -> Option<String> {
    if fs::read_to_string(path).ok().as_deref() != Some(content) {
        fs::create_dir_all(path.parent()?).ok()?;
        fs::write(path, content).ok()?;
    }
    Some(slashed(path))
}

/// Per-user config/cache dir, never a shared temp dir: the shell sources these files.
fn base_dir() -> PathBuf {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let root = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else {
        var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|h| h.join(".cache")))
    };
    root.unwrap_or_else(std::env::temp_dir).join("ywza").join("shell")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(c: &str) -> Option<Shell> {
        detect(c).map(|d| d.1)
    }

    #[test]
    fn detects_bare_known_shells() {
        assert_eq!(kind("pwsh"), Some(Shell::Pwsh));
        assert_eq!(kind("PowerShell.exe"), Some(Shell::Pwsh));
        assert_eq!(kind("/usr/bin/bash"), Some(Shell::Bash));
        assert_eq!(kind("zsh"), Some(Shell::Zsh));
        let (p, s) = detect("\"C:/Program Files/PowerShell/7/pwsh.exe\"").unwrap();
        assert_eq!((p.as_str(), s), ("C:/Program Files/PowerShell/7/pwsh.exe", Shell::Pwsh));
    }

    #[test]
    fn leaves_everything_else_alone() {
        for c in ["cmd", "cmd.exe", "bash -c ls", "pwsh -NoProfile", "claude", "python", ""] {
            assert_eq!(kind(c), None, "{c}");
        }
    }
}
