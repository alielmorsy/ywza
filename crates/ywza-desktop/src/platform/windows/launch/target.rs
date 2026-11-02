use super::app_paths::app_path;
use std::path::{Path, PathBuf};

/// Extensions `CreateProcess` can run itself.
const RUNNABLE: &[&str] = &["exe", "com", "bat", "cmd"];

/// What to start.
#[derive(Debug, PartialEq)]
pub(super) enum Target {
    Exe(PathBuf),
    /// A document, URL or URI: ShellExecute decides what opens it.
    Shell(String),
}

fn canon(p: &Path) -> String {
    p.canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .to_lowercase()
}

pub(super) fn is_self(path: &Path) -> bool {
    std::env::current_exe().is_ok_and(|me| canon(path) == canon(&me))
}

pub(super) fn resolve(program: &str, cwd: Option<&Path>) -> Option<Target> {
    if Path::new(program).is_dir() {
        return Some(Target::Shell(program.to_string())); // opens in Explorer
    }
    let own_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let exts = path_exts(&std::env::var("PATHEXT").unwrap_or_default());
    // symlink_metadata: app execution aliases (WindowsApps) are reparse points.
    let is_file = |p: &Path| p.symlink_metadata().is_ok_and(|m| !m.is_dir());
    resolve_with(program, cwd, &dirs, &exts, own_dir.as_deref(), &is_file).or_else(|| {
        // App Paths: where installers register programs that are not on PATH.
        let name = if has_ext(program) {
            program.to_string()
        } else {
            format!("{program}.exe")
        };
        app_path(&name).filter(|p| is_file(p)).map(Target::Exe)
    })
}

/// The pure part of [`resolve`]: URIs and non-executable files go to the shell,
/// paths are used as given (plus PATHEXT), bare names are searched in `dirs` only.
fn resolve_with(
    program: &str,
    cwd: Option<&Path>,
    dirs: &[PathBuf],
    exts: &[String],
    skip_dir: Option<&Path>,
    is_file: &dyn Fn(&Path) -> bool,
) -> Option<Target> {
    if is_uri(program) {
        return Some(Target::Shell(program.to_string()));
    }
    let classify = |p: PathBuf| match ext(&p) {
        Some(e) if !RUNNABLE.contains(&e.as_str()) => {
            Target::Shell(p.to_string_lossy().into_owned())
        }
        _ => Target::Exe(p),
    };
    let find = |base: &Path| -> Option<PathBuf> {
        if has_ext(&base.to_string_lossy()) && is_file(base) {
            return Some(base.to_path_buf());
        }
        exts.iter()
            .map(|e| PathBuf::from(format!("{}{e}", base.display())))
            .find(|p| is_file(p))
    };
    let given = Path::new(program);
    if given.is_absolute() || program.contains(['\\', '/']) {
        let full = match cwd {
            Some(c) if !given.is_absolute() => c.join(given),
            _ => given.to_path_buf(),
        };
        return find(&full).map(classify);
    }
    // A bare name: a document in the working directory, else a program on PATH.
    if let Some(doc) = cwd.map(|c| c.join(program)) {
        if ext(&doc).is_some_and(|e| !RUNNABLE.contains(&e.as_str())) && is_file(&doc) {
            return Some(classify(doc));
        }
    }
    dirs.iter()
        .filter(|d| !d.as_os_str().is_empty() && skip_dir.is_none_or(|s| canon(d) != canon(s)))
        .find_map(|d| find(&d.join(program)))
        .map(classify)
}

pub(super) fn ext(p: &Path) -> Option<String> {
    p.extension().map(|e| e.to_string_lossy().to_lowercase())
}

fn has_ext(name: &str) -> bool {
    Path::new(name).extension().is_some()
}

/// `PATHEXT` entries (".EXE;.CMD") restricted to what CreateProcess can run.
fn path_exts(pathext: &str) -> Vec<String> {
    let mut v: Vec<String> = pathext
        .split(';')
        .map(str::to_lowercase)
        .filter(|e| e.strip_prefix('.').is_some_and(|e| RUNNABLE.contains(&e)))
        .collect();
    if v.is_empty() {
        v = vec![".com".into(), ".exe".into(), ".bat".into(), ".cmd".into()];
    }
    v
}

/// `scheme:` with a scheme of 2+ characters (a single letter is a drive).
pub(super) fn is_uri(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else {
        return false;
    };
    scheme.len() >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
}

#[cfg(test)]
mod tests {
    use super::super::shell::quote;
    use super::*;
    use std::collections::HashSet;

    fn files(list: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<String> = list.iter().map(|s| s.to_lowercase()).collect();
        move |p: &Path| set.contains(&p.to_string_lossy().to_lowercase())
    }

    fn r(program: &str, dirs: &[&str], skip: Option<&str>, present: &[&str]) -> Option<Target> {
        let dirs: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
        let exts = path_exts(".COM;.EXE;.BAT;.CMD;.VBS");
        resolve_with(
            program,
            Some(Path::new(r"C:\work")),
            &dirs,
            &exts,
            skip.map(Path::new),
            &files(present),
        )
    }

    fn exe(p: &str) -> Option<Target> {
        Some(Target::Exe(PathBuf::from(p)))
    }

    #[test]
    fn bare_names_search_path_and_skip_own_dir() {
        let present = [
            r"C:\mine\notepad.exe",
            r"C:\sys\notepad.exe",
            r"C:\sys\tool.cmd",
        ];
        // The own folder comes first on PATH but is skipped: the real one wins.
        assert_eq!(
            r(
                "notepad",
                &[r"C:\mine", r"C:\sys"],
                Some(r"C:\mine"),
                &present
            ),
            exe(r"C:\sys\notepad.exe")
        );
        assert_eq!(
            r(
                "notepad.exe",
                &[r"C:\mine", r"C:\sys"],
                Some(r"C:\MINE"),
                &present
            ),
            exe(r"C:\sys\notepad.exe")
        );
        // Only the own folder has it: not found, never self.
        assert_eq!(
            r("notepad", &[r"C:\mine"], Some(r"C:\mine"), &present),
            None
        );
        assert_eq!(
            r("tool", &[r"C:\sys"], None, &present),
            exe(r"C:\sys\tool.cmd")
        );
        assert_eq!(r("nothing", &[r"C:\sys"], None, &present), None);
    }

    #[test]
    fn explicit_paths_and_documents() {
        let present = [
            r"C:\a\b.exe",
            r"C:\work\notes.txt",
            r"C:\work\sub\run.exe",
            r"C:\docs\x.pdf",
        ];
        assert_eq!(r(r"C:\a\b", &[], None, &present), exe(r"C:\a\b.exe"));
        assert_eq!(
            r(r"sub\run.exe", &[], None, &present),
            exe(r"C:\work\sub\run.exe")
        );
        assert_eq!(
            r("notes.txt", &[], None, &present),
            Some(Target::Shell(r"C:\work\notes.txt".into()))
        );
        assert_eq!(
            r(r"C:\docs\x.pdf", &[], None, &present),
            Some(Target::Shell(r"C:\docs\x.pdf".into()))
        );
        assert_eq!(r(r"C:\missing\x.pdf", &[], None, &present), None);
    }

    #[test]
    fn uris() {
        assert!(is_uri("ms-settings:"));
        assert!(is_uri("ms-settings:network"));
        assert!(is_uri("https://example.com"));
        assert!(!is_uri(r"C:\x\y.exe"));
        assert!(!is_uri("C:foo"));
        assert!(!is_uri("notepad"));
        assert_eq!(
            r("ms-settings:", &[], None, &[]),
            Some(Target::Shell("ms-settings:".into()))
        );
    }

    #[test]
    fn pathext_and_quoting() {
        assert_eq!(path_exts(".COM;.EXE;.VBS;.MSC"), [".com", ".exe"]);
        assert_eq!(path_exts(""), [".com", ".exe", ".bat", ".cmd"]);
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("ab"), "ab");
    }

    #[test]
    fn real_path_resolves_system_tool_not_self() {
        let Some(Target::Exe(p)) = resolve("cmd", None) else {
            panic!("cmd not found")
        };
        assert!(p.ends_with("cmd.exe") && !is_self(&p));
    }
}
