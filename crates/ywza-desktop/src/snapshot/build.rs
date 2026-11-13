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
        label_unnamed(&mut elements);
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

/// Kinds that are often an unnamed wrapper or hit target of a visible label (web option
/// rows, custom radios listed as `clickable, clickable, text "Option"`).
const BORROWS: &[&str] = &[
    "clickable",
    "maybe-clickable",
    "toggle",
    "selectable",
    "radio",
    "checkbox",
];
/// Named containers that are not labels.
const NOT_LABELS: &[&str] = &["document", "dialog", "ocr"];
const LOOKAHEAD: usize = 4;

/// Gives unnamed actionable elements the name of the node they wrap, the text right of
/// them on the same row, or the overlapping named row just before them.
fn label_unnamed(elements: &mut [Element]) {
    let labels: Vec<Option<String>> = (0..elements.len())
        .map(|i| label_for(elements, i))
        .collect();
    for (e, l) in elements.iter_mut().zip(labels) {
        if let Some(l) = l {
            e.name = l;
            e.labeled = true;
        }
    }
}

fn label_for(els: &[Element], i: usize) -> Option<String> {
    let e = &els[i];
    if !e.name.is_empty() || !BORROWS.contains(&e.kind) {
        return None;
    }
    let named = |o: &&Element| !o.name.is_empty() && !NOT_LABELS.contains(&o.kind);
    let r = e.rect;
    let beside = |o: &Element| {
        let cy = o.rect.center().1;
        o.kind == "text"
            && cy >= r.y
            && cy <= r.y + r.h
            && o.rect.x >= r.x
            && o.rect.x - (r.x + r.w) <= r.h.max(20) * 2
    };
    if let Some(o) = els[i + 1..]
        .iter()
        .take(LOOKAHEAD)
        .filter(named)
        .find(|o| r.intersect(&o.rect) == o.rect || beside(o))
    {
        return Some(o.name.clone());
    }
    let prev = i.checked_sub(1).map(|j| &els[j]).filter(named)?;
    (!prev.rect.intersect(&r).is_empty()).then(|| prev.name.clone())
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
