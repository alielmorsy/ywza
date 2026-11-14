use super::model::{Element, Snapshot};
use super::ocr::ocr_similar;
use std::collections::{HashMap, HashSet};

/// One change between two snapshots; the old/new elements are kept whole so the
/// renderer can pick the short or long text form.
#[derive(Debug, Clone)]
pub enum Delta {
    Added(Element),
    Removed(Element),
    Changed { old: Element, new: Element },
}

/// Identity of an element: its key; OCR lines their id (carried over by position);
/// other keyless elements their kind and name.
#[derive(Hash, PartialEq, Eq)]
enum Ident<'a> {
    Key(&'a [i32]),
    Ocr(u32),
    Named(&'static str, &'a str),
}

fn ident(e: &Element) -> Ident<'_> {
    if e.kind == "ocr" {
        Ident::Ocr(e.id)
    } else if e.key.is_empty() {
        Ident::Named(e.kind, e.name.as_str())
    } else {
        Ident::Key(e.key.as_slice())
    }
}

/// Names equal, or OCR readings of the same text up to noise.
fn same_name(a: &Element, b: &Element) -> bool {
    a.name == b.name || (a.kind == "ocr" && b.kind == "ocr" && ocr_similar(&a.name, &b.name))
}

/// Whether only the value may differ between two matched elements.
pub(super) fn same_shape(a: &Element, b: &Element) -> bool {
    a.kind == b.kind
        && same_name(a, b)
        && a.enabled == b.enabled
        && a.selected == b.selected
        && a.busy == b.busy
}

/// Changes from `old` to `new`, matched by identity, first match wins: removals in
/// old order, then additions and changes in new order.
pub fn diff(old: &Snapshot, new: &Snapshot) -> Vec<Delta> {
    let mut first: HashMap<Ident<'_>, &Element> = HashMap::new();
    for o in &old.elements {
        first.entry(ident(o)).or_insert(o);
    }
    let kept: HashSet<Ident<'_>> = new.elements.iter().map(ident).collect();
    let mut out = Vec::new();
    for o in &old.elements {
        if !kept.contains(&ident(o)) {
            out.push(Delta::Removed(o.clone()));
        }
    }
    for n in &new.elements {
        match first.get(&ident(n)) {
            None => out.push(Delta::Added(n.clone())),
            Some(o) if same_shape(o, n) && o.value == n.value => {}
            Some(&o) => out.push(Delta::Changed {
                old: o.clone(),
                new: n.clone(),
            }),
        }
    }
    out
}
