//! `<config_dir>/config.toml`, kept in memory and persisted on every change.

use std::{collections::HashSet, path::PathBuf, sync::RwLock};

use anyhow::Context;
use tokio::sync::watch;
use ywza_proto::{Config, Peer, Permissions};

pub struct ConfigStore {
    path: PathBuf,
    inner: RwLock<Config>,
    /// Bumped on every change, so connections re-check their peer's permissions.
    changed: watch::Sender<()>,
}

impl ConfigStore {
    /// Loads the file, or writes the defaults when it does not exist.
    pub fn load(path: PathBuf) -> anyhow::Result<Self> {
        let mut config = match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("parsing {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(e.into()),
        };
        migrate(&mut config);
        let store = Self {
            path,
            inner: RwLock::new(config),
            changed: watch::channel(()).0,
        };
        store.save()?;
        Ok(store)
    }

    pub fn get(&self) -> Config {
        self.inner.read().unwrap().clone()
    }

    /// One peer's entry, without cloning the whole config.
    pub fn peer(&self, id: &str) -> Option<Peer> {
        self.inner.read().unwrap().peer(id).cloned()
    }

    /// Wakes on every later change.
    pub fn subscribe(&self) -> watch::Receiver<()> {
        self.changed.subscribe()
    }

    /// Replaces the config. A blank `api_key` keeps the stored one; legacy `trusted_peers` migrate.
    pub fn set(&self, new: Config) -> anyhow::Result<()> {
        self.update(|c| {
            *c = merge(c, new);
            migrate(c);
        })
    }

    /// Adds a newly paired peer with the default permissions (no-op if present).
    pub fn trust(&self, id: String, name: String) -> anyhow::Result<()> {
        self.update(|c| {
            if c.peer(&id).is_none() {
                c.peers.push(Peer {
                    id,
                    name,
                    perms: Permissions::default(),
                })
            }
        })
    }

    pub fn update(&self, f: impl FnOnce(&mut Config)) -> anyhow::Result<()> {
        f(&mut self.inner.write().unwrap());
        self.changed.send_replace(());
        self.save()
    }

    fn save(&self) -> anyhow::Result<()> {
        let text = toml::to_string(&*self.inner.read().unwrap())?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.path, text).with_context(|| format!("writing {}", self.path.display()))
    }
}

fn merge(old: &Config, mut new: Config) -> Config {
    if new.llm.api_key.trim().is_empty() {
        new.llm.api_key = old.llm.api_key.clone();
    }
    new
}

/// Moves legacy `trusted_peers` into `peers` with every permission (they could do everything
/// before), and drops duplicate ids (first entry wins).
fn migrate(c: &mut Config) {
    for id in std::mem::take(&mut c.trusted_peers) {
        if c.peer(&id).is_none() {
            c.peers.push(Peer {
                name: id.chars().take(8).collect(),
                id,
                perms: Permissions::all(),
            });
        }
    }
    let mut seen = HashSet::new();
    c.peers.retain(|p| seen.insert(p.id.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_api_key_keeps_stored_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path().join("config.toml")).unwrap();
        let mut c = store.get();
        c.llm.api_key = "sk-1".into();
        store.set(c).unwrap();

        let mut masked = store.get().masked();
        masked.llm.model = "m2".into();
        store.set(masked).unwrap();
        assert_eq!(store.get().llm.api_key, "sk-1");
        assert_eq!(store.get().llm.model, "m2");

        let mut c = store.get();
        c.llm.api_key = "sk-2".into();
        store.set(c).unwrap();
        let reloaded = ConfigStore::load(dir.path().join("config.toml")).unwrap();
        assert_eq!(reloaded.get().llm.api_key, "sk-2");
        assert_eq!(reloaded.get().llm.model, "m2");
    }

    #[test]
    fn legacy_trusted_peers_become_full_peers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "trusted_peers = [\"abcdef0123456789\", \"abcdef0123456789\", \"known\"]\n\
             [[peers]]\nid = \"known\"\nname = \"phone\"\ncontrol = false\n",
        )
        .unwrap();
        let store = ConfigStore::load(path.clone()).unwrap();
        let c = store.get();
        assert!(c.trusted_peers.is_empty());
        assert_eq!(c.peers.len(), 2);
        let p = c.peer("abcdef0123456789").unwrap();
        assert_eq!((p.name.as_str(), p.perms), ("abcdef01", Permissions::all()));
        assert!(
            !c.peer("known").unwrap().perms.control,
            "existing entry kept"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("trusted_peers"), "{text}");

        // An old client sending only `trusted_peers` through set_config.
        let old = Config {
            trusted_peers: vec!["x".into()],
            ..Default::default()
        };
        store.set(old).unwrap();
        assert_eq!(store.get().peer("x").unwrap().perms, Permissions::all());

        let mut rx = store.subscribe();
        store.trust("y".into(), "laptop".into()).unwrap();
        assert!(rx.has_changed().unwrap());
        let y = store.peer("y").unwrap();
        assert_eq!(
            (y.name.as_str(), y.perms),
            ("laptop", Permissions::default())
        );
    }
}
