use super::model::{Element, Snapshot};
use std::fmt::Write;

/// Above this many removed (or added) elements, that side is summarised.
const COLLAPSE: usize = 20;
const SAMPLE_REMOVED: usize = 8;
const SAMPLE_ADDED: usize = 12;

/// Changes from `old` to `new`, matched by key: `+[20] dialog "Save as"`, `-[12]`,
/// `~[3] = "new value"`. Other keyless elements match on kind and name; OCR lines on their id
/// (carried over by position), and a re-read that differs only by OCR noise is no change.
/// A page change (many removed or added) starts with a count line and lists a sample.
pub fn diff(old: &Snapshot, new: &Snapshot) -> String {
    let ocr = |a: &Element, b: &Element| a.kind == "ocr" && b.kind == "ocr";
    let same = |a: &Element, b: &Element| {
        if ocr(a, b) {
            a.id == b.id
        } else if a.key.is_empty() || b.key.is_empty() {
            a.key.is_empty() && b.key.is_empty() && a.kind == b.kind && a.name == b.name
        } else {
            a.key == b.key
        }
    };
    let same_name = |a: &Element, b: &Element| {
        a.name == b.name || (ocr(a, b) && super::ocr::ocr_similar(&a.name, &b.name))
    };
    let removed: Vec<String> = old
        .elements
        .iter()
        .filter(|o| !new.elements.iter().any(|n| same(o, n)))
        .map(|o| format!("-[{}]", o.id))
        .collect();
    // In document order; true = added.
    let mut rest: Vec<(bool, String)> = Vec::new();
    for n in &new.elements {
        match old.elements.iter().find(|o| same(o, n)) {
            None => rest.push((true, format!("+{}", n.line()))),
            Some(o)
                if o.kind == n.kind
                    && same_name(o, n)
                    && o.enabled == n.enabled
                    && o.selected == n.selected
                    && o.busy == n.busy =>
            {
                if o.value != n.value {
                    rest.push((false, format!("~[{}]{}", n.id, n.value_text())));
                }
            }
            Some(_) => rest.push((false, format!("~{}", n.line()))),
        }
    }
    let r = removed.len();
    let a = rest.iter().filter(|(added, _)| *added).count();
    let mut out = String::new();
    if r > COLLAPSE || a > COLLAPSE {
        let _ = writeln!(
            out,
            "{r} removed, {a} added (sample; get_elements lists all)"
        );
    }
    if r > COLLAPSE {
        let _ = writeln!(out, "{} …", removed[..SAMPLE_REMOVED].join(" "));
    } else {
        for l in &removed {
            let _ = writeln!(out, "{l}");
        }
    }
    let mut shown = 0;
    for (added, l) in &rest {
        if *added && a > COLLAPSE {
            shown += 1;
            if shown > SAMPLE_ADDED {
                continue;
            }
        }
        let _ = writeln!(out, "{l}");
    }
    if a > COLLAPSE {
        let _ = writeln!(out, "+… {} more added", a - SAMPLE_ADDED);
    }
    if out.is_empty() {
        out.push_str("(no changes)\n");
    }
    out
}
