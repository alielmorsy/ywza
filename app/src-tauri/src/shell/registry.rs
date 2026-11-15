//! Writes and removes the verbs under HKEY_CURRENT_USER.

use windows_registry::{Result, CURRENT_USER};

use super::layout::{verb, Key, ROOTS};

pub fn install(keys: &[Key]) -> Result<()> {
    for k in keys {
        let key = CURRENT_USER.create(&k.path)?;
        for (name, value) in &k.values {
            key.set_string(name, value)?;
        }
    }
    Ok(())
}

pub fn remove() -> Result<()> {
    for root in ROOTS {
        let path = verb(root);
        if CURRENT_USER.open(&path).is_ok() {
            CURRENT_USER.remove_tree(&path)?;
        }
    }
    Ok(())
}

pub fn installed(keys: &[Key]) -> bool {
    keys.iter().all(|k| {
        CURRENT_USER.open(&k.path).is_ok_and(|key| {
            k.values
                .iter()
                .all(|(name, value)| key.get_string(name).is_ok_and(|v| &v == value))
        })
    })
}
