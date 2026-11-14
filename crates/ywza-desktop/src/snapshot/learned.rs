//! Labels the model gave to unnamed elements, applied to every later snapshot of the app.

use super::anchor::{resolve, Anchor, Resolve};
use super::model::Snapshot;
use std::sync::RwLock;

static LEARNED: RwLock<Vec<(Anchor, String)>> = RwLock::new(Vec::new());

/// Replaces the whole table (the agent owns persistence).
pub fn set_learned(labels: Vec<(Anchor, String)>) {
    *LEARNED.write().unwrap_or_else(|e| e.into_inner()) = labels;
}

/// Names the elements the table resolves to, never over a real name.
pub(super) fn apply(snap: &mut Snapshot) {
    let table = LEARNED.read().unwrap_or_else(|e| e.into_inner());
    if table.is_empty() {
        return;
    }
    let process = snap.window.process.clone();
    for (anchor, label) in table
        .iter()
        .filter(|(a, l)| !l.is_empty() && a.process.eq_ignore_ascii_case(&process))
    {
        let Resolve::Found(found) = resolve(anchor, snap) else {
            continue;
        };
        if let Some(e) = snap.elements.iter_mut().find(|e| e.id == found[0].id) {
            if e.anchor.name.is_empty() {
                e.name = label.clone();
                e.learned = true;
            }
        }
    }
}
