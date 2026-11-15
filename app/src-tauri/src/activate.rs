//! Single instance: a second launch hands its arguments to the running app and exits. Explorer
//! starts one process per selected item, so selections arriving close together become one dock open.

use std::{collections::HashSet, path::Path, sync::Mutex, thread, time::Duration};

use tauri::{AppHandle, Manager};

use crate::{
    dock::context::{self, ContextItem},
    shell::layout::ARG,
};

pub const MAX_ITEMS: usize = 50;
const GATHER: Duration = Duration::from_millis(300);
// shortcut: a fixed wait for the dock webview to start listening, replace with a ready signal if it proves flaky.
const COLD_START: Duration = Duration::from_secs(3);

/// Paths gathered so far and a counter of launches; only the last launch's timer flushes.
static PENDING: Mutex<(Vec<String>, u64)> = Mutex::new((Vec::new(), 0));

/// The raw Explorer paths after `--explorer`, or none when the launch was not from Explorer.
pub fn explorer_args(argv: &[String]) -> &[String] {
    argv.iter()
        .position(|a| a == ARG)
        .map_or(&[], |i| &argv[i + 1..])
}

/// Paths are untrusted: keep absolute ones that exist, once each, at most `MAX_ITEMS`.
pub fn items(paths: &[String]) -> Vec<ContextItem> {
    let mut seen = HashSet::new();
    paths
        .iter()
        .filter(|p| Path::new(p).is_absolute() && seen.insert(p.to_lowercase()))
        .filter_map(|p| {
            let dir = std::fs::metadata(p).ok()?.is_dir();
            Some(ContextItem::Path { path: p.clone(), dir })
        })
        .take(MAX_ITEMS)
        .collect()
}

fn gather(app: &AppHandle, paths: &[String], wait: Duration) {
    let gen = {
        let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        p.0.extend_from_slice(paths);
        p.1 += 1;
        p.1
    };
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(wait);
        let paths = {
            let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
            if p.1 != gen {
                return;
            }
            std::mem::take(&mut p.0)
        };
        let items = items(&paths);
        if !items.is_empty() {
            context::open(&app, "explorer", items);
        }
    });
}

/// Single-instance callback: runs in the first instance with a later launch's arguments.
pub fn forwarded(app: &AppHandle, argv: Vec<String>, _cwd: String) {
    let paths = explorer_args(&argv);
    if !paths.is_empty() {
        return gather(app, paths, GATHER);
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The first instance's own arguments, once the app is set up.
pub fn launched(app: &AppHandle) {
    let argv: Vec<String> = std::env::args().collect();
    let paths = explorer_args(&argv);
    if !paths.is_empty() {
        gather(app, paths, COLD_START);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_explorer_args() {
        assert!(explorer_args(&args(&["ywza-app.exe"])).is_empty());
        assert!(explorer_args(&args(&["ywza-app.exe", "--explorer"])).is_empty());
        assert_eq!(
            explorer_args(&args(&["ywza-app.exe", "--explorer", r"C:\a b.txt"])),
            args(&[r"C:\a b.txt"])
        );
    }

    #[test]
    fn validates_and_caps() {
        let dir = std::env::temp_dir().join(format!("ywza-activate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut paths = vec![
            "relative.txt".to_string(),
            dir.join("missing.txt").to_string_lossy().into_owned(),
            dir.to_string_lossy().into_owned(),
            dir.to_string_lossy().to_uppercase(),
        ];
        for i in 0..60 {
            let f = dir.join(format!("{i}.txt"));
            std::fs::write(&f, "").unwrap();
            paths.push(f.to_string_lossy().into_owned());
        }
        let got = items(&paths);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got.len(), MAX_ITEMS);
        assert_eq!(got[0], ContextItem::Path { path: paths[2].clone(), dir: true });
        assert_eq!(got[1], ContextItem::Path { path: paths[4].clone(), dir: false });
    }
}
