use super::model::{Element, Snapshot};
use std::fmt::Write;

/// Changes from `old` to `new`, matched by key: `+[20] dialog "Save as"`, `-[12]`,
/// `~[3] = "new value"`. Elements without a key (OCR) match on kind and name.
pub fn diff(old: &Snapshot, new: &Snapshot) -> String {
    let same = |a: &Element, b: &Element| {
        if a.key.is_empty() || b.key.is_empty() {
            a.key.is_empty() && b.key.is_empty() && a.kind == b.kind && a.name == b.name
        } else {
            a.key == b.key
        }
    };
    let mut out = String::new();
    for o in &old.elements {
        if !new.elements.iter().any(|n| same(o, n)) {
            let _ = writeln!(out, "-[{}]", o.id);
        }
    }
    for n in &new.elements {
        match old.elements.iter().find(|o| same(o, n)) {
            None => {
                let _ = writeln!(out, "+{}", n.line());
            }
            Some(o) if o.kind == n.kind && o.name == n.name && o.enabled == n.enabled => {
                if o.value != n.value {
                    let _ = writeln!(out, "~[{}]{}", n.id, n.value_text());
                }
            }
            Some(_) => {
                let _ = writeln!(out, "~{}", n.line());
            }
        }
    }
    if out.is_empty() {
        out.push_str("(no changes)\n");
    }
    out
}
