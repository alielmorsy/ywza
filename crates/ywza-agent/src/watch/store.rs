//! `watchers.json` in the agent dir: loaded once, rewritten atomically on every change.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

use super::model::Watcher;
use crate::atomic::write_atomic;

pub const MAX_WATCHERS: usize = 50;
const ID_LEN: usize = 5;
const ID_CHARS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

pub struct Store {
    path: PathBuf,
    list: Mutex<Vec<Watcher>>,
}

static STORE: OnceLock<Store> = OnceLock::new();

/// Opens `<dir>/watchers.json`; the first call wins. True when this call did.
pub fn init(dir: &Path) -> bool {
    if STORE.get().is_some() {
        return false;
    }
    STORE.set(Store::load(dir.join("watchers.json"))).is_ok()
}

pub fn get() -> Result<&'static Store, String> {
    STORE.get().ok_or_else(|| "watchers are not available here".into())
}

fn new_id() -> String {
    let mut b = [0u8; ID_LEN];
    getrandom::fill(&mut b).expect("OS random source");
    b.iter()
        .map(|&x| ID_CHARS[x as usize % ID_CHARS.len()] as char)
        .collect()
}

impl Store {
    /// A file that does not parse is moved to `watchers.json.bad` so the next save keeps it.
    pub fn load(path: PathBuf) -> Self {
        let list = match std::fs::read_to_string(&path) {
            Ok(t) => serde_json::from_str(&t).unwrap_or_else(|e| {
                tracing::error!(error = %e, "watchers.json unreadable, starting empty");
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        Self {
            path,
            list: Mutex::new(list),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Watcher>> {
        self.list.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn list(&self) -> Vec<Watcher> {
        self.lock().clone()
    }

    /// Runs `f` on a copy of the list and keeps the result only if it saved.
    fn edit<T>(&self, f: impl FnOnce(&mut Vec<Watcher>) -> Result<T, String>) -> Result<T, String> {
        let mut g = self.lock();
        let mut next = g.clone();
        let out = f(&mut next)?;
        let text = serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?;
        write_atomic(&self.path, &text).map_err(|e| format!("saving watchers: {e}"))?;
        *g = next;
        Ok(out)
    }

    /// Stores `w` under a fresh id and returns it.
    pub fn add(&self, mut w: Watcher) -> Result<Watcher, String> {
        self.edit(|list| {
            if list.len() >= MAX_WATCHERS {
                return Err(format!("{MAX_WATCHERS} watchers already; delete one first"));
            }
            w.id = loop {
                let id = new_id();
                if !list.iter().any(|x| x.id == id) {
                    break id;
                }
            };
            list.push(w.clone());
            Ok(w)
        })
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<Watcher, String> {
        self.edit(|list| {
            let w = find(list, id)?;
            w.enabled = enabled;
            Ok(w.clone())
        })
    }

    pub fn remove(&self, id: &str) -> Result<Watcher, String> {
        self.edit(|list| {
            let i = list.iter().position(|w| w.id == id);
            Ok(list.remove(i.ok_or_else(|| no_watcher(id))?))
        })
    }

    pub fn record_fire(&self, id: &str, now_ms: u64) {
        let r = self.edit(|list| {
            let w = find(list, id)?;
            w.fired += 1;
            w.last_fired_ms = Some(now_ms);
            Ok(())
        });
        if let Err(e) = r {
            tracing::warn!(error = %e, id, "recording a watcher fire failed");
        }
    }
}

fn no_watcher(id: &str) -> String {
    format!("no watcher `{id}`; watcher_list shows the ids")
}

fn find<'a>(list: &'a mut [Watcher], id: &str) -> Result<&'a mut Watcher, String> {
    list.iter_mut().find(|w| w.id == id).ok_or_else(|| no_watcher(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watch::model::{Action, Condition};

    fn watcher() -> Watcher {
        Watcher {
            id: String::new(),
            name: "w".into(),
            enabled: true,
            when: Condition::WindowAppears { title: "x".into() },
            then: Action::Notify { text: None },
            min_gap_s: 60,
            max_per_hour: 12,
            fired: 0,
            last_fired_ms: None,
        }
    }

    #[test]
    fn changes_persist_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("watchers.json");
        let s = Store::load(path.clone());
        let w = s.add(watcher()).unwrap();
        assert_eq!(w.id.len(), ID_LEN);
        s.record_fire(&w.id, 5);
        s.set_enabled(&w.id, false).unwrap();
        let again = Store::load(path.clone()).list();
        assert_eq!(again.len(), 1);
        assert!(!again[0].enabled);
        assert_eq!((again[0].fired, again[0].last_fired_ms), (1, Some(5)));
        s.remove(&w.id).unwrap();
        assert!(Store::load(path).list().is_empty());
        assert!(s.remove(&w.id).unwrap_err().contains("no watcher"));
    }

    #[test]
    fn caps_the_count_and_keeps_ids_unique() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::load(dir.path().join("watchers.json"));
        for _ in 0..MAX_WATCHERS {
            s.add(watcher()).unwrap();
        }
        assert!(s.add(watcher()).unwrap_err().contains("delete one"));
        let mut ids: Vec<_> = s.list().into_iter().map(|w| w.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), MAX_WATCHERS);
    }

    #[test]
    fn a_broken_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("watchers.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(Store::load(path).list().is_empty());
        assert!(dir.path().join("watchers.json.bad").exists());
    }
}
