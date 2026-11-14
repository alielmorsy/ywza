use super::anchor::{norm, rel, Anchor, Tree};
use super::classify::classify;
use super::learned;
use super::model::{Element, Raw, Snapshot};
use crate::platform::Handle;
use crate::{Rect, WindowInfo};
use std::collections::HashMap;

impl Snapshot {
    /// Classifies raw nodes and numbers them. Elements matching one in `prev`
    /// (same key) keep their ID, so IDs stay meaningful across re-snapshots.
    pub(crate) fn build(
        window: WindowInfo,
        raws: Vec<(Raw, Option<Handle>)>,
        prev: Option<&Snapshot>,
    ) -> Snapshot {
        let n = raws.len();
        let tree = Tree::new(raws.iter().map(|(r, _)| r));
        let mut listed: Vec<(usize, Element)> = raws
            .into_iter()
            .enumerate()
            .filter_map(|(i, (raw, handle))| {
                classify(&raw, &window.rect).map(|e| {
                    let anchor = Anchor {
                        process: window.process.clone(),
                        role: raw.role,
                        automation_id: raw.automation_id.clone(),
                        name: norm(&e.name),
                        path: tree.path(i),
                        rel: rel(&raw.rect, raw.bounds.as_ref().unwrap_or(&window.rect)),
                    };
                    (i, Element { handle, anchor, ..e })
                })
            })
            .collect();
        label_unnamed(&mut listed);
        drop_redundant_text(&mut listed);
        let (src, mut elements): (Vec<usize>, Vec<Element>) = listed.into_iter().unzip();
        let mut next = prev.map_or(0, |p| p.elements.iter().map(|e| e.id).max().unwrap_or(0));
        let mut carried: HashMap<&[i32], u32> = HashMap::new();
        for o in prev.iter().flat_map(|p| &p.elements) {
            if !o.key.is_empty() {
                carried.entry(o.key.as_slice()).or_insert(o.id);
            }
        }
        for e in &mut elements {
            e.id = match carried.get(e.key.as_slice()) {
                Some(&id) if !e.key.is_empty() => id,
                _ => {
                    next += 1;
                    next
                }
            };
        }
        let mut slot: Vec<Option<usize>> = vec![None; n];
        for (k, &i) in src.iter().enumerate() {
            slot[i] = Some(k);
        }
        let mut nearest: Vec<Option<usize>> = vec![None; n];
        for i in 0..n {
            nearest[i] = tree
                .parent(i)
                .and_then(|p| if slot[p].is_some() { Some(p) } else { nearest[p] });
        }
        let parents: Vec<Option<u32>> = src
            .iter()
            .map(|&i| nearest[i].and_then(|p| slot[p]).map(|k| elements[k].id))
            .collect();
        for (e, p) in elements.iter_mut().zip(parents) {
            e.parent = p;
        }
        let mut snap = Snapshot {
            window,
            seq: prev.map_or(1, |p| p.seq + 1),
            elements,
        };
        learned::apply(&mut snap);
        snap
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
fn label_unnamed(elements: &mut [(usize, Element)]) {
    let labels: Vec<Option<String>> = (0..elements.len())
        .map(|i| label_for(elements, i))
        .collect();
    for ((_, e), l) in elements.iter_mut().zip(labels) {
        if let Some(l) = l {
            e.name = l;
            e.labeled = true;
        }
    }
}

fn label_for(els: &[(usize, Element)], i: usize) -> Option<String> {
    let e = &els[i].1;
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
        .map(|(_, o)| o)
        .take(LOOKAHEAD)
        .filter(named)
        .find(|o| r.intersect(&o.rect) == o.rect || beside(o))
    {
        return Some(o.name.clone());
    }
    let prev = i.checked_sub(1).map(|j| &els[j].1).filter(named)?;
    (!prev.rect.intersect(&r).is_empty()).then(|| prev.name.clone())
}

/// Text inside another element whose name already contains it (the label inside a
/// button or tab) adds nothing.
fn drop_redundant_text(elements: &mut Vec<(usize, Element)>) {
    let containers: Vec<(Rect, String)> = elements
        .iter()
        .map(|(_, e)| e)
        .filter(|e| e.kind != "text" && !e.name.is_empty())
        .map(|e| (e.rect, e.name.clone()))
        .collect();
    elements.retain(|(_, e)| {
        e.kind != "text"
            || !containers
                .iter()
                .any(|(r, n)| n.contains(&e.name) && r.intersect(&e.rect) == e.rect)
    });
}
