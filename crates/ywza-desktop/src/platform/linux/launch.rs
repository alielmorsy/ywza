//! Launching: executables run directly (PATH search by the OS), documents, folders
//! and URIs go through `xdg-open`.

use super::x;
use crate::{Error, Launched, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What to start.
#[derive(Debug, PartialEq)]
enum Target {
    Exe(String),
    /// A document, folder or URI: `xdg-open` decides what opens it.
    Open(String),
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Kind {
    Dir,
    Exe,
    Doc,
}

/// `scheme:` with a scheme of 2+ characters (`https:`, `mailto:`), never a path.
fn is_uri(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else {
        return false;
    };
    scheme.len() >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
}

/// Pure decision: `fs` says whether a path is a directory / an executable file / a file.
fn classify(program: &str, cwd: Option<&Path>, fs: &dyn Fn(&Path) -> Option<Kind>) -> Target {
    if is_uri(program) {
        return Target::Open(program.to_string());
    }
    let given = Path::new(program);
    let full: PathBuf = match cwd {
        Some(c) if !given.is_absolute() => c.join(given),
        _ => given.to_path_buf(),
    };
    // A bare name is a program on PATH unless a document of that name sits in cwd.
    let bare = !program.contains('/');
    match fs(&full) {
        Some(Kind::Dir) => Target::Open(full.to_string_lossy().into_owned()),
        Some(Kind::Doc) => Target::Open(full.to_string_lossy().into_owned()),
        Some(Kind::Exe) if !bare => Target::Exe(full.to_string_lossy().into_owned()),
        _ => Target::Exe(program.to_string()),
    }
}

fn kind_on_disk(p: &Path) -> Option<Kind> {
    let m = p.metadata().ok()?;
    Some(if m.is_dir() {
        Kind::Dir
    } else if m.permissions().mode() & 0o111 != 0 {
        Kind::Exe
    } else {
        Kind::Doc
    })
}

/// First executable file called `name` in `dirs`.
fn in_dirs(name: &str, dirs: &[PathBuf], is_exe: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| is_exe(p))
}

/// The executable `launch` would start for `program`, without starting it. None for
/// documents, folders and URIs (`xdg-open` handles those) and unknown names.
pub fn find_program(program: &str) -> Option<String> {
    let is_exe = |p: &Path| kind_on_disk(p) == Some(Kind::Exe);
    let Target::Exe(exe) = classify(program, None, &kind_on_disk) else {
        return None;
    };
    if exe.contains('/') {
        return is_exe(Path::new(&exe)).then_some(exe);
    }
    let dirs: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH")?).collect();
    in_dirs(&exe, &dirs, &is_exe).map(|p| p.to_string_lossy().into_owned())
}

/// Starts a program, document or URI. For `xdg-open` targets `Launched::pid` is the
/// opener, which usually exits at once; `wait_for_window` then accepts any new window.
pub fn launch(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Launched> {
    let before = x::list_windows()?.into_iter().map(|w| w.id).collect();
    let mut cmd = match classify(program, cwd, &kind_on_disk) {
        Target::Exe(exe) => {
            let mut c = Command::new(exe);
            c.args(args);
            c
        }
        Target::Open(what) => {
            let mut c = Command::new("xdg-open");
            c.arg(what);
            c
        }
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.envs(crate::A11Y_ENV.iter().copied());
    let mut child = cmd
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(format!("program {program}")),
            _ => Error::Os(format!("launch {program}: {e}")),
        })?;
    let pid = child.id();
    // Reap it when it exits, so `alive` does not see a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(Launched { pid, before })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs(files: &[(&str, Kind)]) -> impl Fn(&Path) -> Option<Kind> {
        let v: Vec<(String, Kind)> = files.iter().map(|(p, k)| (p.to_string(), *k)).collect();
        move |p: &Path| v.iter().find(|(f, _)| Path::new(f) == p).map(|(_, k)| *k)
    }

    fn c(program: &str, files: &[(&str, Kind)]) -> Target {
        classify(program, Some(Path::new("/work")), &fs(files))
    }

    #[test]
    fn path_search_takes_the_first_executable() {
        let dirs = [PathBuf::from("/a"), PathBuf::from("/b")];
        let is_exe = |p: &Path| p == Path::new("/b/tool") || p == Path::new("/a/other");
        assert_eq!(in_dirs("tool", &dirs, &is_exe), Some(PathBuf::from("/b/tool")));
        assert_eq!(in_dirs("none", &dirs, &is_exe), None);
    }

    #[test]
    fn uris_documents_and_programs() {
        assert!(is_uri("https://example.com"));
        assert!(is_uri("mailto:a@b.c"));
        assert!(!is_uri("/usr/bin/x"));
        assert!(!is_uri("./a:b"));
        assert!(!is_uri("x"));
        assert_eq!(
            c("https://example.com", &[]),
            Target::Open("https://example.com".into())
        );
        assert_eq!(c("gedit", &[]), Target::Exe("gedit".into()));
        assert_eq!(
            c("notes.txt", &[("/work/notes.txt", Kind::Doc)]),
            Target::Open("/work/notes.txt".into())
        );
        assert_eq!(
            c("/docs", &[("/docs", Kind::Dir)]),
            Target::Open("/docs".into())
        );
        assert_eq!(
            c("./run.sh", &[("/work/./run.sh", Kind::Exe)]),
            Target::Exe("/work/./run.sh".into())
        );
        // An executable file named like a PATH program in cwd is not run: PATH wins.
        assert_eq!(
            c("tool", &[("/work/tool", Kind::Exe)]),
            Target::Exe("tool".into())
        );
    }
}
