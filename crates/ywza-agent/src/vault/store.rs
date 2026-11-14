use std::{collections::BTreeMap, path::Path};

use anyhow::Context;

use crate::atomic::write_atomic;

pub const FILE: &str = "vault.toml";

pub type Entries = BTreeMap<String, String>;

/// Reads `vault.toml` from `dir`; a missing file is an empty vault.
pub fn load(dir: &Path) -> anyhow::Result<Entries> {
    let path = dir.join(FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("parsing {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Entries::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn save(dir: &Path, entries: &Entries) -> anyhow::Result<()> {
    let path = dir.join(FILE);
    #[cfg(unix)]
    owner_only(&path.with_extension("tmp"))?;
    write_atomic(&path, &toml::to_string(entries)?)
        .with_context(|| format!("writing {}", path.display()))
}

/// `write_atomic` writes into this temp file and renames it, so it keeps these permissions.
#[cfg(unix)]
fn owner_only(tmp: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(tmp)?;
    std::fs::set_permissions(tmp, std::fs::Permissions::from_mode(0o600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
        let mut e = Entries::new();
        e.insert("github".into(), "ghp_a\"b\\c d".into());
        e.insert("my key".into(), "x".into());
        save(dir.path(), &e).unwrap();
        assert_eq!(load(dir.path()).unwrap(), e);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn a_broken_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), "a = [").unwrap();
        assert!(load(dir.path()).is_err());
    }
}
