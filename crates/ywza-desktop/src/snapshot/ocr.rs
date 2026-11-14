use super::anchor::{norm, rel, Anchor};
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
            selected: false,
            busy: false,
            group: String::new(),
            labeled: false,
            learned: false,
            rect,
            patterns: Patterns::default(),
            key: Vec::new(),
            popup: false,
            parent: None,
            anchor: Anchor {
                process: snapshot.window.process.clone(),
                role: "ocr",
                name: norm(&text),
                rel: rel(&rect, &snapshot.window.rect),
                ..Anchor::default()
            },
            handle: None,
        });
    }
    snapshot.elements.extend(added);
}

/// Whether `want` appears in `got` (both lowercase, no spaces) up to OCR noise: one wrong,
/// missing or extra character per 4 (re-reading the same pixels gives "File eView" for
/// "File View"). Short text (under 4 characters) must match exactly.
pub fn ocr_matches(got: &str, want: &str) -> bool {
    let (g, w): (Vec<char>, Vec<char>) = (got.chars().collect(), want.chars().collect());
    if w.is_empty() {
        return g.is_empty();
    }
    // Edit distance of `want` against the best-matching substring of `got`.
    let mut row: Vec<usize> = vec![0; g.len() + 1];
    for (i, wc) in w.iter().enumerate() {
        let mut next = vec![i + 1; g.len() + 1];
        for (j, gc) in g.iter().enumerate() {
            next[j + 1] = (row[j] + usize::from(wc != gc))
                .min(row[j + 1] + 1)
                .min(next[j] + 1);
        }
        row = next;
    }
    row.into_iter().min().unwrap() * 4 <= w.len()
}

/// Lowercase without whitespace, the form [`ocr_matches`] compares.
pub fn ocr_norm(s: &str) -> String {
    s.split_whitespace().collect::<String>().to_lowercase()
}

/// Two OCR readings of the same text: equal up to noise, or one a cut-off reading of the
/// other ("Use the current in" for "Use the current implied DC point"), at least a third of it.
pub fn ocr_similar(a: &str, b: &str) -> bool {
    let (a, b) = (ocr_norm(a), ocr_norm(b));
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    let n = short.chars().count();
    if n * 3 < long.chars().count() || (n < 4 && short != long) {
        return false;
    }
    ocr_matches(&long, &short)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_match_tolerates_noise() {
        assert!(ocr_matches("fileeview", "fileview")); // one stray char
        assert!(ocr_matches("xfilex", "file")); // neighbours caught by the padding
        assert!(!ocr_matches("saveas", "loadlut")); // different text
        assert!(!ocr_matches("go", "no")); // short text: exact only
        assert!(!ocr_matches("", "file"));
        assert!(ocr_matches("", ""));
    }

    #[test]
    fn ocr_similar_accepts_jitter_and_cut_off_readings() {
        assert!(ocr_similar(
            "Use the current implied...",
            "Use the current in"
        ));
        assert!(ocr_similar("File eView", "File View"));
        assert!(!ocr_similar(
            "Use the current implied DC operating point",
            "Use"
        ));
        assert!(!ocr_similar("Save", "Load"));
        assert!(!ocr_similar("go", "gone"));
        assert!(ocr_similar("OK", "ok"));
    }
}
