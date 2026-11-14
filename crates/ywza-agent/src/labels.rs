//! Names the model gave to unnamed elements, kept in `<dir>/labels.json` and pushed to
//! ywza-desktop, which applies them to every snapshot of the app.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use ywza_desktop::Anchor;

use crate::anchor::SavedAnchor;

static DIR: OnceLock<PathBuf> = OnceLock::new();
static LABELS: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    anchor: SavedAnchor,
    label: String,
}

fn file(dir: &Path) -> PathBuf {
    dir.join("labels.json")
}

fn push(entries: &[Entry]) {
    ywza_desktop::set_learned(
        entries
            .iter()
            .map(|e| (e.anchor.to_anchor(), e.label.clone()))
            .collect(),
    );
}

/// Sets the agent dir, loads its labels and hands them to ywza-desktop; the first call wins.
pub fn init(dir: &Path) {
    if DIR.set(dir.to_path_buf()).is_err() {
        return;
    }
    let loaded: Vec<Entry> = std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    push(&loaded);
    *LABELS.lock().unwrap() = loaded;
}

/// Stores `label` for `anchor`; an empty label forgets it.
pub fn learn(anchor: &Anchor, label: &str) -> anyhow::Result<()> {
    let saved = SavedAnchor::from(anchor);
    let label = label.trim();
    let mut all = LABELS.lock().unwrap();
    let mut next: Vec<Entry> = all.iter().filter(|e| e.anchor != saved).cloned().collect();
    if !label.is_empty() {
        next.push(Entry { anchor: saved, label: label.into() });
    }
    if let Some(dir) = DIR.get() {
        let text = serde_json::to_string_pretty(&next)?;
        crate::atomic::write_atomic(&file(dir), &text).context("saving labels.json")?;
    }
    push(&next);
    *all = next;
    Ok(())
}

pub fn list() -> Vec<(SavedAnchor, String)> {
    LABELS
        .lock()
        .unwrap()
        .iter()
        .map(|e| (e.anchor.clone(), e.label.clone()))
        .collect()
}
