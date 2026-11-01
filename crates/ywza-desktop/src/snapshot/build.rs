use super::classify::classify;
use super::model::{Element, Raw, Snapshot};
use crate::platform::Handle;
use crate::{Rect, WindowInfo};

impl Snapshot {
    /// Classifies raw nodes and numbers them. Elements matching one in `prev`
    /// (same key) keep their ID, so IDs stay meaningful across re-snapshots.
    pub(crate) fn build(
        window: WindowInfo,
        raws: Vec<(Raw, Option<Handle>)>,
        prev: Option<&Snapshot>,
    ) -> Snapshot {
        let mut elements: Vec<Element> = raws
            .into_iter()
            .filter_map(|(raw, handle)| {
                classify(&raw, &window.rect).map(|e| Element { handle, ..e })
            })
            .collect();
        drop_redundant_text(&mut elements);
        let mut next = prev.map_or(0, |p| p.elements.iter().map(|e| e.id).max().unwrap_or(0));
        for e in &mut elements {
            let carried = prev.and_then(|p| {
                p.elements
                    .iter()
                    .find(|o| !o.key.is_empty() && o.key == e.key)
            });
            e.id = match carried {
                Some(o) => o.id,
                None => {
                    next += 1;
                    next
                }
            };
        }
        Snapshot {
            window,
            seq: prev.map_or(1, |p| p.seq + 1),
            elements,
        }
    }

    /// Same elements as `other`, ignoring the snapshot number.
    pub fn same_content(&self, other: &Snapshot) -> bool {
        self.elements.len() == other.elements.len()
            && self
                .elements
                .iter()
                .zip(&other.elements)
                .all(|(a, b)| a.line() == b.line())
    }

    pub fn get(&self, id: u32) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }
}

/// Text inside another element whose name already contains it (the label inside a
/// button or tab) adds nothing.
fn drop_redundant_text(elements: &mut Vec<Element>) {
    let containers: Vec<(Rect, String)> = elements
        .iter()
        .filter(|e| e.kind != "text" && !e.name.is_empty())
        .map(|e| (e.rect, e.name.clone()))
        .collect();
    elements.retain(|e| {
        e.kind != "text"
            || !containers
                .iter()
                .any(|(r, n)| n.contains(&e.name) && r.intersect(&e.rect) == e.rect)
    });
}
