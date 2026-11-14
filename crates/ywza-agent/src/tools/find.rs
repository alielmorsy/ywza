//! Clicking what is on screen without an element id: visible text (an element's name, else
//! OCR words), menu paths ("ADT > Sizing > Show plots") and points of a screenshot.
//!
//! Menus are where element ids fail: OCR merges neighbouring titles ("Options ADT"), ids churn
//! on every repaint, and toolbars and Motif menus often ignore the keyboard. A path is walked
//! by text instead, each item looked for first in what the previous click opened.

use std::time::{Duration, Instant};

use ywza_desktop::{self as desk, Button, Image, Rect, Snapshot, WindowId};

use super::{
    act::{settle_diff, NO_CHANGES},
    ocr::{engine, read_lines, snapshot_with_ocr},
    output::{desk_err, MODEL_IMAGE_MAX, R},
    util::blocking,
    Ctx, ToolOutput,
};

const PATH_SEP: &str = " > ";
/// A menu that has not opened yet gets this much longer before a path step gives up.
const SLOW_MENU: Duration = Duration::from_millis(700);
const SETTLE_MAX: Duration = Duration::from_secs(2);

/// A place to click: an element of the snapshot, or text read from the pixels.
#[derive(Clone, Debug)]
struct Spot {
    text: String,
    rect: Rect,
    element: Option<u32>,
}

/// `"A > B > C"` as its items; text without a full path (e.g. "Next >") stays one item.
fn split_path(text: &str) -> Vec<&str> {
    let parts: Vec<&str> = text.split(PATH_SEP).map(str::trim).collect();
    if parts.len() > 1 && parts.iter().all(|p| !p.is_empty()) {
        parts
    } else {
        vec![text.trim()]
    }
}

/// Element names compare without case, spaces, mnemonics and trailing dots ("Save _As…").
fn label_norm(s: &str) -> String {
    desk::ocr_norm(s)
        .trim_end_matches(['.', '…', ':'])
        .replace(['&', '_'], "")
}

/// 0 = same text, 1 = same up to OCR noise or a cut-off reading. Elements must match exactly.
fn quality(spot: &Spot, label: &str) -> Option<u8> {
    if label_norm(&spot.text) == label_norm(label) {
        Some(0)
    } else if spot.element.is_none() && desk::ocr_similar(&spot.text, label) {
        Some(1)
    } else {
        None
    }
}

fn same_spot(a: &Spot, b: &Spot) -> bool {
    let ((ax, ay), (bx, by)) = (a.rect.center(), b.rect.center());
    (ax - bx).abs() <= 8 && (ay - by).abs() <= 8 && label_norm(&a.text) == label_norm(&b.text)
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    let r = (a.x + a.w).max(b.x + b.w);
    let bottom = (a.y + a.h).max(b.y + b.h);
    Rect::new(x, y, r - x, bottom - y)
}

/// Every run of up to `k + 1` consecutive words of each line, so one word of a merged
/// line ("ADT" of "Options ADT") gets its own box.
fn word_spots(lines: &[ywza_ocr::TextLine], k: usize) -> Vec<Spot> {
    let rect = |r: &ywza_ocr::Rect| Rect::new(r.x, r.y, r.w, r.h);
    let mut out = Vec::new();
    for l in lines {
        if l.words.is_empty() {
            out.push(Spot {
                text: l.text.clone(),
                rect: rect(&l.rect),
                element: None,
            });
            continue;
        }
        for i in 0..l.words.len() {
            for j in i + 1..=(i + k + 1).min(l.words.len()) {
                let ws = &l.words[i..j];
                out.push(Spot {
                    text: ws
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    rect: ws.iter().map(|w| rect(&w.rect)).reduce(union).unwrap(),
                    element: None,
                });
            }
        }
    }
    out
}

fn element_spots(snap: &Snapshot) -> Vec<Spot> {
    snap.elements
        .iter()
        .filter(|e| e.kind != "ocr" && e.enabled && !e.name.is_empty())
        .map(|e| Spot {
            text: e.name.clone(),
            rect: e.rect,
            element: Some(e.id),
        })
        .collect()
}

/// The best matches of `label`, in reading order. With `before` (what was on screen before
/// the last click), matches that appeared since come first: the menu that click opened.
fn matches<'a>(spots: &'a [Spot], label: &str, before: Option<&[Spot]>) -> Vec<&'a Spot> {
    let hits: Vec<(u8, &Spot)> = spots
        .iter()
        .filter_map(|s| Some((quality(s, label)?, s)))
        .collect();
    let Some(best) = hits.iter().map(|h| h.0).min() else {
        return Vec::new();
    };
    let mut hits: Vec<&Spot> = hits
        .into_iter()
        .filter(|h| h.0 == best)
        .map(|h| h.1)
        .collect();
    if let Some(before) = before {
        let fresh: Vec<&Spot> = hits
            .iter()
            .copied()
            .filter(|h| !before.iter().any(|b| same_spot(b, h)))
            .collect();
        if !fresh.is_empty() {
            hits = fresh;
        }
    }
    hits.sort_by_key(|s| (s.rect.y, s.rect.x));
    hits.dedup_by(|a, b| a.rect == b.rect);
    hits
}

/// What is on the window now: its snapshot (stored as the latest, OCR lines merged when it
/// has too few elements), element spots, and OCR word spots when no element is `label`.
async fn look(cx: &Ctx, w: WindowId, label: &str) -> Result<(Snapshot, Vec<Spot>), ToolOutput> {
    let (snap, _) = snapshot_with_ocr(w, cx.snap(w), false, cx.ocr)
        .await
        .map_err(desk_err)?;
    cx.store(w, snap.clone());
    let mut spots = element_spots(&snap);
    if matches(&spots, label, None).is_empty() {
        // Same pixels as the snapshot's OCR, so usually an OCR cache hit.
        let lines = read_lines(w, engine(cx.ocr, false))
            .await
            .map_err(|e| ToolOutput::err(format!("could not read the window's text: {e}")))?;
        spots.extend(word_spots(&lines, label.split_whitespace().count()));
    }
    Ok((snap, spots))
}

/// The window's pixels; None when it cannot be captured (closed, minimized).
pub async fn frame(w: WindowId) -> Option<Image> {
    blocking(move || desk::capture_window(w).ok()).await
}

/// Whether two captures differ by more than a blinking text cursor: the changed pixels
/// must span more than 4 px both ways (a spinner does, a caret does not).
pub fn visibly_changed(a: &Image, b: &Image) -> bool {
    if (a.width, a.height, a.origin) != (b.width, b.height, b.origin) {
        return true;
    }
    let width = a.width as usize;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    let (pa, pb) = (a.rgba.as_chunks::<4>().0, b.rgba.as_chunks::<4>().0);
    for (i, _) in pa.iter().zip(pb).enumerate().filter(|(_, (p, q))| p != q) {
        let (x, y) = (i % width, i / width);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    x0 != usize::MAX && x1 - x0 >= 4 && y1 - y0 >= 4
}

/// Waits until two captures 150 ms apart show no visible change, at most `max`. Works where
/// the element tree cannot tell (OCR-only windows, menus drawn by the app).
async fn settle_pixels(w: WindowId, max: Duration) {
    let deadline = Instant::now() + max;
    // Give a slow app time to start reacting before "unchanged" counts as settled.
    tokio::time::sleep(Duration::from_millis(250)).await;
    let Some(mut last) = frame(w).await else {
        return;
    };
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let Some(now) = frame(w).await else { return };
        if !visibly_changed(&last, &now) {
            return;
        }
        last = now;
    }
}

async fn click_spot(snap: Snapshot, spot: Spot, button: Button, count: u32) -> desk::Result<()> {
    let w = snap.window.id;
    blocking(move || match spot.element {
        Some(id) => desk::click(&snap, id, button, count),
        None => desk::click_at(w, spot.rect.center(), button, count),
    })
    .await
}

/// After an action without element ids: "ok" in a batch, else the changes since `base`.
async fn finish(cx: &Ctx, w: WindowId, base: Option<Snapshot>, what: String) -> R {
    if cx.quiet {
        return Ok(ToolOutput::ok("ok"));
    }
    settle_pixels(w, SETTLE_MAX).await;
    Ok(ToolOutput::ok(match base {
        Some(base) => match settle_diff(cx, w, &base).await {
            Some(d) if d == NO_CHANGES => format!("{what}; no visible change"),
            Some(d) => format!("{what}; changes:\n{}", d.trim_end()),
            None => what,
        },
        None => format!("{what}; get_elements to see the window now"),
    }))
}

/// Clicks visible text, or walks a menu path item by item.
pub async fn click_text(
    cx: &Ctx,
    w: WindowId,
    text: &str,
    nth: Option<u32>,
    button: Button,
    count: u32,
) -> R {
    let path = split_path(text);
    let mut base: Option<Snapshot> = None;
    let mut before: Option<Vec<Spot>> = None;
    let mut what = String::new();
    for (i, label) in path.iter().enumerate() {
        let last = i + 1 == path.len();
        let (mut snap, mut spots) = look(cx, w, label).await?;
        let mut hits = matches(&spots, label, before.as_deref());
        if hits.is_empty() && i > 0 {
            tokio::time::sleep(SLOW_MENU).await;
            (snap, spots) = look(cx, w, label).await?;
            hits = matches(&spots, label, before.as_deref());
        }
        let spot = match pick(&hits, label, if last { nth } else { None }, i > 0) {
            Ok(s) => s.clone(),
            Err(e) if i > 0 => {
                let opened = shown_since(&spots, before.as_deref().unwrap_or_default());
                // Close what this path opened, so the window is as it was.
                let esc = vec!["escape"; i].join(" ");
                let _ = blocking(move || desk::press_keys(Some(w), &esc)).await;
                return Err(ToolOutput::err(format!(
                    "{e} after clicking {:?}; it showed: {opened}. Closed it again with Escape.",
                    path[..i].join(PATH_SEP)
                )));
            }
            Err(e) => return Err(ToolOutput::err(e)),
        };
        base.get_or_insert_with(|| snap.clone());
        what = format!("clicked {:?} at {},{}", spot.text, spot.rect.x, spot.rect.y);
        let (b, c) = if last {
            (button, count)
        } else {
            (Button::Left, 1)
        };
        click_spot(snap, spot, b, c).await.map_err(desk_err)?;
        if !last {
            settle_pixels(w, SETTLE_MAX).await;
        }
        before = Some(spots);
    }
    finish(cx, w, base, what).await
}

fn pick<'a>(
    hits: &[&'a Spot],
    label: &str,
    nth: Option<u32>,
    in_menu: bool,
) -> Result<&'a Spot, String> {
    match (nth, hits) {
        (_, []) => Err(format!("no {label:?} on screen")),
        (Some(n), _) => hits
            .get(n.max(1) as usize - 1)
            .copied()
            .ok_or_else(|| format!("{label:?} is on screen {} times, no match {n}", hits.len())),
        (None, [one]) => Ok(*one),
        // Inside a menu that just opened: its first match.
        (None, [first, ..]) if in_menu => Ok(*first),
        (None, _) => {
            let list: Vec<String> = hits
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}) {:?} at {},{}", i + 1, s.text, s.rect.x, s.rect.y))
                .collect();
            Err(format!(
                "{label:?} is on screen {} times; pass nth: {}",
                hits.len(),
                list.join(", ")
            ))
        }
    }
}

/// The text that appeared since `before`, one entry per element or OCR line.
fn shown_since(spots: &[Spot], before: &[Spot]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for s in spots
        .iter()
        .filter(|s| !before.iter().any(|b| same_spot(b, s)))
    {
        // Word runs overlap; keep the longest reading of each place.
        if !seen.iter().any(|t| t.contains(s.text.as_str())) {
            seen.retain(|t| !s.text.contains(t));
            seen.push(&s.text);
        }
    }
    if seen.is_empty() {
        return "nothing new".into();
    }
    seen.truncate(40);
    seen.join(" | ")
}

/// Clicks a point given in pixels of the window's screenshot (scaled like `screenshot` scales it).
pub async fn click_point(cx: &Ctx, w: WindowId, x: i32, y: i32, button: Button, count: u32) -> R {
    let img = blocking(move || desk::capture_window(w))
        .await
        .map_err(desk_err)?;
    let long = img.width.max(img.height).max(1);
    let scale = long as f64 / long.min(MODEL_IMAGE_MAX) as f64;
    let (px, py) = (
        (x as f64 * scale).round() as i32,
        (y as f64 * scale).round() as i32,
    );
    if px < 0 || py < 0 || px >= img.width as i32 || py >= img.height as i32 {
        let (sw, sh) = (
            (img.width as f64 / scale) as u32,
            (img.height as f64 / scale) as u32,
        );
        return Err(ToolOutput::err(format!(
            "{x},{y} is outside w{w}'s screenshot ({sw}x{sh})"
        )));
    }
    let at = (img.origin.0 + px, img.origin.1 + py);
    let base = cx.snap(w);
    blocking(move || desk::click_at(w, at, button, count))
        .await
        .map_err(desk_err)?;
    finish(cx, w, base, format!("clicked at {x},{y}")).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(text: &str, x: i32, y: i32) -> Spot {
        Spot {
            text: text.into(),
            rect: Rect::new(x, y, 40, 16),
            element: None,
        }
    }

    #[test]
    fn a_blinking_caret_is_no_change() {
        let img = || Image {
            width: 40,
            height: 30,
            rgba: vec![255; 40 * 30 * 4],
            origin: (0, 0),
        };
        let (a, mut caret, mut text) = (img(), img(), img());
        for y in 5..25 {
            caret.rgba[(y * 40 + 10) * 4] = 0; // 1 px wide, 20 px tall
        }
        for (x, y) in [(3, 3), (20, 12)] {
            text.rgba[(y * 40 + x) * 4] = 0;
        }
        assert!(!visibly_changed(&a, &a));
        assert!(!visibly_changed(&a, &caret));
        assert!(visibly_changed(&a, &text));
    }

    #[test]
    fn paths_split_only_when_complete() {
        assert_eq!(
            split_path("ADT > Sizing > Show plots"),
            ["ADT", "Sizing", "Show plots"]
        );
        assert_eq!(split_path("Next >"), ["Next >"]);
        assert_eq!(split_path(" File "), ["File"]);
    }

    #[test]
    fn a_word_of_a_merged_line_gets_its_own_box() {
        let word = |t: &str, x| ywza_ocr::Word {
            text: t.into(),
            rect: ywza_ocr::Rect {
                x,
                y: 0,
                w: 50,
                h: 16,
            },
        };
        let line = ywza_ocr::TextLine {
            text: "Options ADT".into(),
            rect: ywza_ocr::Rect {
                x: 0,
                y: 0,
                w: 110,
                h: 16,
            },
            words: vec![word("Options", 0), word("ADT", 60)],
        };
        let spots = word_spots(&[line], 1);
        let hits = matches(&spots, "ADT", None);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rect.x, 60);
        assert!(matches(&spots, "Options ADT", None)[0].rect.w == 110);
    }

    #[test]
    fn menu_items_that_just_opened_win() {
        let before = vec![spot("Sizing", 10, 100)];
        let now = vec![spot("Sizing", 10, 100), spot("Sizing", 200, 30)];
        let hits = matches(&now, "Sizing", Some(&before));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rect.x, 200);
        let all = matches(&now, "Sizing", None);
        assert!(pick(&all, "Sizing", None, false)
            .unwrap_err()
            .contains("pass nth"));
        assert_eq!(pick(&all, "Sizing", Some(2), false).unwrap().rect.y, 100);
    }

    #[test]
    fn matching_tolerates_ocr_but_not_for_elements() {
        assert_eq!(quality(&spot("Fle", 0, 0), "File"), None); // 3 chars: too short to guess
        assert_eq!(quality(&spot("Open plots", 0, 0), "Open plots"), Some(0));
        assert_eq!(
            quality(
                &spot("Import ADT Design Pont", 0, 0),
                "Import ADT Design Point"
            ),
            Some(1)
        );
        let mut el = spot("Save all", 0, 0);
        el.element = Some(3);
        assert_eq!(quality(&el, "Save"), None);
        el.text = "Save _As…".into();
        assert_eq!(quality(&el, "save as"), Some(0));
    }

    #[test]
    fn shown_since_keeps_the_longest_reading() {
        let before = vec![spot("File", 0, 0)];
        let now = vec![
            spot("File", 0, 0),
            spot("Show", 0, 50),
            spot("Show plots", 0, 50),
        ];
        assert_eq!(shown_since(&now, &before), "Show plots");
    }
}
