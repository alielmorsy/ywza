//! Which elements become lens chips, and what changed between two lists.

use std::collections::{HashMap, HashSet};

use ywza_desktop::{Element, Patterns, Rect};
use ywza_proto::ElementInfo;

use crate::inspect::rel_rect;

/// More chips than a phone screen can show are noise.
pub const MAX: usize = 300;

/// Kinds a person taps: control types and the pattern-based kinds of `ywza-desktop`.
const TAPPABLE: &[&str] = &[
    "button",
    "splitbutton",
    "edit",
    "editable",
    "checkbox",
    "radio",
    "combobox",
    "link",
    "menuitem",
    "tabitem",
    "listitem",
    "treeitem",
    "toggle",
    "selectable",
    "expandable",
    "clickable",
];

/// A text field whose content can be replaced. Documents (editors) count only when writable.
pub fn editable(kind: &str, p: &Patterns) -> bool {
    match kind {
        "edit" | "editable" => !p.value_readonly,
        "document" | "combobox" => p.value && !p.value_readonly,
        _ => false,
    }
}

/// The chip for one element, or None when it is not worth tapping: disabled, unnamed
/// (unless a text field) or without area.
fn chip(e: &Element, origin: (i32, i32)) -> Option<ElementInfo> {
    chip_of(e.id, e.kind, &e.name, e.enabled, &e.patterns, e.rect, origin)
}

fn chip_of(
    id: u32,
    kind: &str,
    name: &str,
    enabled: bool,
    patterns: &Patterns,
    rect: Rect,
    origin: (i32, i32),
) -> Option<ElementInfo> {
    let editable = editable(kind, patterns);
    if !enabled || rect.w <= 0 || rect.h <= 0 {
        return None;
    }
    if !editable && (!TAPPABLE.contains(&kind) || name.trim().is_empty()) {
        return None;
    }
    Some(ElementInfo {
        id,
        role: kind.into(),
        name: name.into(),
        rect: rel_rect(origin, rect),
        editable,
    })
}

/// The chips of a snapshot's elements, in document order, at most [`MAX`].
pub fn project(elements: &[Element], origin: (i32, i32)) -> Vec<ElementInfo> {
    elements
        .iter()
        .filter_map(|e| chip(e, origin))
        .take(MAX)
        .collect()
}

/// What the client must apply to go from `sent` to `now`: chips that are new or differ, and
/// the ids that vanished (ascending).
pub fn changes(
    sent: &HashMap<u32, ElementInfo>,
    now: &[ElementInfo],
) -> (Vec<ElementInfo>, Vec<u32>) {
    let upsert = now
        .iter()
        .filter(|c| sent.get(&c.id) != Some(c))
        .cloned()
        .collect();
    let live: HashSet<u32> = now.iter().map(|c| c.id).collect();
    let mut removed: Vec<u32> = sent.keys().filter(|id| !live.contains(id)).copied().collect();
    removed.sort_unstable();
    (upsert, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(id: u32, kind: &str, name: &str) -> Option<ElementInfo> {
        chip_of(
            id,
            kind,
            name,
            true,
            &Patterns::default(),
            Rect::new(110, 220, 30, 10),
            (100, 200),
        )
    }

    #[test]
    fn keeps_named_tappables_relative_to_the_monitor() {
        let c = make(3, "button", "OK").unwrap();
        let r = c.rect;
        assert_eq!((r.x, r.y, r.width, r.height), (10, 20, 30, 10));
        assert!(!c.editable);
        assert!(make(3, "text", "Label").is_none());
        assert!(make(3, "button", "  ").is_none());
        assert!(make(3, "ocr", "OK").is_none());
    }

    #[test]
    fn text_fields_are_editable_even_unnamed() {
        let c = make(4, "edit", "").unwrap();
        assert!(c.editable);
        let ro = Patterns {
            value: true,
            value_readonly: true,
            ..Patterns::default()
        };
        assert!(!editable("edit", &ro));
        assert!(!editable("document", &Patterns::default()));
        let rw = Patterns {
            value: true,
            ..Patterns::default()
        };
        assert!(editable("document", &rw));
    }

    #[test]
    fn disabled_and_flat_elements_are_skipped() {
        let p = Patterns::default();
        let at = |enabled, r| chip_of(1, "button", "A", enabled, &p, r, (0, 0));
        assert!(at(true, Rect::new(0, 0, 0, 5)).is_none());
        assert!(at(false, Rect::new(0, 0, 5, 5)).is_none());
        assert!(at(true, Rect::new(0, 0, 5, 5)).is_some());
    }

    #[test]
    fn changes_upsert_differences_and_drop_vanished() {
        let a = make(1, "button", "A").unwrap();
        let b = make(2, "button", "B").unwrap();
        let sent: HashMap<u32, ElementInfo> = [(1, a.clone()), (2, b.clone())].into();
        let (up, gone) = changes(&sent, &[a.clone(), b.clone()]);
        assert!(up.is_empty() && gone.is_empty());
        let moved = ElementInfo {
            name: "B2".into(),
            ..b
        };
        let c = make(3, "link", "C").unwrap();
        let (up, gone) = changes(&sent, &[moved.clone(), c.clone()]);
        assert_eq!(up, vec![moved, c]);
        assert_eq!(gone, vec![1]);
    }
}
