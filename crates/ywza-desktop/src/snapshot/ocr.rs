use super::classify::INFORMATIVE;
use super::model::{Element, Patterns, Snapshot};
use crate::Rect;

/// True when UIA found too little to act on (fewer than 3 named interactive
/// elements), so the caller should add OCR lines with [`merge_ocr`].
pub fn needs_ocr(snapshot: &Snapshot) -> bool {
    let named = snapshot
        .elements
        .iter()
        .filter(|e| !e.name.is_empty() && e.kind != "ocr" && !INFORMATIVE.contains(&e.kind))
        .count();
    named < 3
}

/// Adds OCR text lines as `ocr` elements, dropping any line that lies >50% inside
/// a listed element that already carries text (name or value). Unnamed
/// containers do not swallow OCR text, since that text is exactly what UIA missed.
pub fn merge_ocr(snapshot: &mut Snapshot, lines: Vec<(String, Rect)>) {
    let mut next = snapshot.elements.iter().map(|e| e.id).max().unwrap_or(0);
    let mut added = Vec::new();
    for (text, rect) in lines {
        let covered = snapshot.elements.iter().any(|e| {
            (!e.name.is_empty() || e.value.is_some())
                && e.rect.intersect(&rect).area() * 2 > rect.area()
        });
        if covered || text.trim().is_empty() {
            continue;
        }
        next += 1;
        added.push(Element {
            id: next,
            kind: "ocr",
            name: text.trim().to_string(),
            automation_id: String::new(),
            value: None,
            enabled: true,
            rect,
            patterns: Patterns::default(),
            key: Vec::new(),
            popup: false,
            handle: None,
        });
    }
    snapshot.elements.extend(added);
}
