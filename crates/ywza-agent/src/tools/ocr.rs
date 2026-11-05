//! OCR text as elements: merging into snapshots and re-checking an OCR element before acting.

use ywza_desktop::{self as desk, Image, Rect, Snapshot, WindowId};
use ywza_proto::OcrEngine;

use super::{output::crop, util::blocking};

pub fn engine(configured: OcrEngine, accurate: bool) -> ywza_ocr::Engine {
    match (accurate, configured) {
        (true, _) | (false, OcrEngine::Paddle) => ywza_ocr::Engine::Paddle,
        (false, OcrEngine::Windows) => ywza_ocr::Engine::Windows,
        (false, OcrEngine::Auto) => ywza_ocr::Engine::Auto,
    }
}

/// Snapshot (ids carried over from `prev`) plus OCR text when [`desk::needs_ocr`]. The string is a
/// note for the model when OCR could not run.
pub async fn snapshot_with_ocr(
    w: WindowId,
    prev: Option<Snapshot>,
    accurate: bool,
    ocr: OcrEngine,
) -> desk::Result<(Snapshot, String)> {
    let (mut snap, prev) =
        blocking(move || desk::snapshot(w, prev.as_ref()).map(|s| (s, prev))).await?;
    if !(accurate || desk::needs_ocr(&snap)) {
        return Ok((snap, String::new()));
    }
    let mut lines = ocr_lines(w, engine(ocr, accurate)).await;
    if lines.is_err() && accurate {
        // Paddle may not be built in; the configured engine is better than nothing.
        lines = ocr_lines(w, engine(ocr, false)).await;
    }
    let note = match lines {
        Ok(lines) => {
            desk::merge_ocr(&mut snap, lines);
            carry_ocr_ids(prev.as_ref(), &mut snap);
            String::new()
        }
        Err(e) => format!("(OCR unavailable: {e})\n"),
    };
    Ok((snap, note))
}

/// Re-merges OCR text into `fresh` so the diff does not report it all as removed.
pub async fn keep_ocr(w: WindowId, ocr: OcrEngine, old: &Snapshot, fresh: &mut Snapshot) {
    if let Ok(lines) = ocr_lines(w, engine(ocr, false)).await {
        desk::merge_ocr(fresh, lines);
        carry_ocr_ids(Some(old), fresh);
    }
}

/// OCR text lines of a window in screen coordinates.
async fn ocr_lines(w: WindowId, engine: ywza_ocr::Engine) -> Result<Vec<(String, Rect)>, String> {
    blocking(move || {
        let img = desk::capture_window(w).map_err(|e| e.to_string())?;
        // OCR boxes are image pixels; the capture's origin maps them to screen pixels like element rects.
        let (ox, oy) = img.origin;
        let lines = ywza_ocr::recognize(&to_ocr(img), engine).map_err(|e| e.to_string())?;
        Ok(lines
            .iter()
            .flat_map(|l| l.segments())
            .map(|(text, r)| (text, Rect::new(r.x + ox, r.y + oy, r.w, r.h)))
            .collect())
    })
    .await
}

fn to_ocr(img: Image) -> ywza_ocr::Image {
    ywza_ocr::Image {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    }
}

/// OCR elements have no stable key, so `merge_ocr` numbers them afresh. Give each one the id
/// of the previous snapshot's OCR element with the same text, and new ones ids above all used.
fn carry_ocr_ids(prev: Option<&Snapshot>, snap: &mut Snapshot) {
    let Some(prev) = prev else { return };
    let max = |s: &Snapshot| s.elements.iter().map(|e| e.id).max().unwrap_or(0);
    let mut next = max(prev).max(max(snap));
    let mut taken: Vec<u32> = Vec::new();
    for e in snap.elements.iter_mut().filter(|e| e.kind == "ocr") {
        let old = prev
            .elements
            .iter()
            .find(|o| o.kind == "ocr" && o.name == e.name && !taken.contains(&o.id));
        e.id = match old {
            Some(o) => o.id,
            None => {
                next += 1;
                next
            }
        };
        taken.push(e.id);
    }
}

/// Before acting on an OCR element: re-capture, re-OCR its box and compare the text.
pub async fn check_ocr(w: WindowId, rect: Rect, want: String, ocr: OcrEngine) -> Result<(), String> {
    let norm = |s: &str| s.split_whitespace().collect::<String>().to_lowercase();
    let got = blocking(move || {
        let img = desk::capture_window(w).map_err(|e| e.to_string())?;
        let pad = 4;
        let r = Rect::new(
            rect.x - img.origin.0 - pad,
            rect.y - img.origin.1 - pad,
            rect.w + 2 * pad,
            rect.h + 2 * pad,
        );
        let part = crop(&img, r).ok_or("off screen")?;
        let lines = ywza_ocr::recognize(&part, engine(ocr, false)).map_err(|e| e.to_string())?;
        Ok::<_, String>(
            lines
                .into_iter()
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join(" "),
        )
    })
    .await?;
    if ocr_matches(&norm(&got), &norm(&want)) {
        Ok(())
    } else {
        Err(format!("text there is now {:?}", got.trim()))
    }
}

/// Whether `want` appears in `got` up to OCR noise: one wrong, missing or extra
/// character per 4 (re-reading the same pixels gives "File eView" for "File View").
/// Short text (under 4 characters) must match exactly.
fn ocr_matches(got: &str, want: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_match_tolerates_noise() {
        assert!(ocr_matches("fileeview", "fileview")); // one stray char
        assert!(ocr_matches("xfilex", "file")); // neighbours caught by the padding
        assert!(ocr_matches("loadlut", "loadlut"));
        assert!(!ocr_matches("saveas", "loadlut")); // different text: stale
        assert!(!ocr_matches("go", "no")); // short text: exact only
        assert!(!ocr_matches("", "file"));
        assert!(ocr_matches("", ""));
    }

    #[test]
    fn ocr_engine_choice() {
        assert_eq!(engine(OcrEngine::Windows, true), ywza_ocr::Engine::Paddle);
        assert_eq!(engine(OcrEngine::Auto, false), ywza_ocr::Engine::Auto);
    }
}
