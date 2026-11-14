//! OCR text as elements: merging into snapshots and re-checking an OCR element before acting.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

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
        tracing::warn!("accurate OCR failed, using the configured engine");
        lines = ocr_lines(w, engine(ocr, false)).await;
    }
    let note = match lines {
        Ok(lines) => {
            desk::merge_ocr(&mut snap, lines);
            carry(w, prev.as_ref(), &mut snap);
            String::new()
        }
        Err(e) => {
            tracing::warn!(error = %e, "OCR unavailable");
            format!("(OCR unavailable: {e})\n")
        }
    };
    Ok((snap, note))
}

/// Re-merges OCR text into `fresh` so the diff does not report it all as removed.
pub async fn keep_ocr(w: WindowId, ocr: OcrEngine, old: &Snapshot, fresh: &mut Snapshot) {
    if let Ok(lines) = ocr_lines(w, engine(ocr, false)).await {
        desk::merge_ocr(fresh, lines);
        carry(w, Some(old), fresh);
    }
}

/// OCR text pieces of a window in screen coordinates: lines, split where the pixels show
/// a wide gap between words (menu bar titles).
async fn ocr_lines(w: WindowId, engine: ywza_ocr::Engine) -> Result<Vec<(String, Rect)>, String> {
    let (lines, img) = read(w, engine).await?;
    Ok(lines.iter().flat_map(|l| pieces(l, &img)).collect())
}

/// OCR lines (with their words) of a window, moved to screen coordinates.
pub async fn read_lines(
    w: WindowId,
    engine: ywza_ocr::Engine,
) -> Result<Vec<ywza_ocr::TextLine>, String> {
    read(w, engine).await.map(|(lines, _)| lines)
}

/// The lines plus the capture they were read from.
async fn read(
    w: WindowId,
    engine: ywza_ocr::Engine,
) -> Result<(Vec<ywza_ocr::TextLine>, Image), String> {
    blocking(move || {
        let img = desk::capture_window(w).map_err(|e| e.to_string())?;
        // OCR boxes are image pixels; the capture's origin maps them to screen pixels like element rects.
        let (ox, oy) = img.origin;
        let mut lines = ywza_ocr::recognize(&to_ocr(&img), engine).map_err(|e| e.to_string())?;
        for l in &mut lines {
            for r in std::iter::once(&mut l.rect).chain(l.words.iter_mut().map(|w| &mut w.rect)) {
                r.x += ox;
                r.y += oy;
            }
        }
        Ok((lines, img))
    })
    .await
}

fn to_ocr(img: &Image) -> ywza_ocr::Image {
    ywza_ocr::Image {
        width: img.width,
        height: img.height,
        rgba: img.rgba.clone(),
    }
}

/// A line cut where its words sit far apart on screen, measured in `img` (the capture it was
/// read from): a blank run wider than ~0.45 of the line box height (about a letter height;
/// a space is far narrower) starts a new piece, e.g. menu bar titles "Options   ADT". Each
/// piece's box is its ink, so its centre is on its text. No cut: [`ywza_ocr::TextLine::segments`].
fn pieces(l: &ywza_ocr::TextLine, img: &Image) -> Vec<(String, Rect)> {
    let to_rect = |r: &ywza_ocr::Rect| Rect::new(r.x, r.y, r.w, r.h);
    let fallback = || -> Vec<(String, Rect)> {
        l.segments()
            .into_iter()
            .map(|(t, r)| (t, to_rect(&r)))
            .collect()
    };
    let (ox, oy) = img.origin;
    let line = to_rect(&l.rect);
    let b = Rect::new(line.x - ox, line.y - oy, line.w, line.h).intersect(&Rect::new(
        0,
        0,
        img.width as i32,
        img.height as i32,
    ));
    if l.words.len() < 2 || b.is_empty() {
        return fallback();
    }
    // Middle half of the box only: OCR boxes are padded, and a menu bar's border or shadow
    // line along the top or bottom would mark every column as ink.
    let (y0, y1) = (b.y + b.h / 4, (b.y + b.h * 3 / 4).max(b.y + b.h / 4 + 1));
    let lum = |x: i32, y: i32| {
        let i = (y as usize * img.width as usize + x as usize) * 4;
        let p = &img.rgba[i..i + 3];
        (p[0] as u32 * 3 + p[1] as u32 * 6 + p[2] as u32) / 10
    };
    // Background: the most common shade in the band. Ink: any pixel well off it.
    let mut hist = [0u32; 256];
    for y in y0..y1 {
        for x in b.x..b.x + b.w {
            hist[lum(x, y) as usize] += 1;
        }
    }
    let bg = (0..256).max_by_key(|&v| hist[v]).unwrap_or(0) as i32;
    let ink: Vec<bool> = (b.x..b.x + b.w)
        .map(|x| (y0..y1).any(|y| (lum(x, y) as i32 - bg).abs() > 48))
        .collect();
    let col = |sx: i32| (sx - ox - b.x).clamp(0, b.w) as usize;
    let min_gap = ((b.h * 45 / 100).max(8)) as usize;
    // (first word of the new piece, column where it starts)
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for k in 0..l.words.len() - 1 {
        let (a, c) = (
            col(to_rect(&l.words[k].rect).center().0),
            col(to_rect(&l.words[k + 1].rect).center().0),
        );
        let (mut best, mut run) = ((0, 0), 0);
        for x in a..c {
            run = if ink[x] { 0 } else { run + 1 };
            if run > best.1 {
                best = (x + 1 - run, run);
            }
        }
        if best.1 >= min_gap {
            cuts.push((k + 1, best.0 + best.1 / 2));
        }
    }
    if cuts.is_empty() {
        return fallback();
    }
    let words: Vec<usize> = std::iter::once(0)
        .chain(cuts.iter().map(|c| c.0))
        .chain([l.words.len()])
        .collect();
    let cols: Vec<usize> = std::iter::once(0)
        .chain(cuts.iter().map(|c| c.1))
        .chain([ink.len()])
        .collect();
    (0..words.len() - 1)
        .map(|i| {
            let ws = &l.words[words[i]..words[i + 1]];
            let text = ws
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let inked: Vec<usize> = (cols[i]..cols[i + 1]).filter(|&x| ink[x]).collect();
            let rect = match (inked.first(), inked.last()) {
                (Some(&x0), Some(&x1)) => {
                    Rect::new(ox + b.x + x0 as i32, line.y, (x1 - x0 + 1) as i32, line.h)
                }
                _ => Rect::new(line.x, line.y, line.w, line.h),
            };
            (text, rect)
        })
        .collect()
}

/// An OCR id handed out before, remembered by text and place.
struct Seen {
    id: u32,
    name: String,
    rect: Rect,
}

/// Per window, so text that comes back where it was (a menu reopened, a dialog shown again)
/// gets its old id even though the previous snapshot did not have it.
static SEEN: LazyLock<Mutex<HashMap<WindowId, Vec<Seen>>>> = LazyLock::new(Mutex::default);
const SEEN_MAX: usize = 400;

fn carry(w: WindowId, prev: Option<&Snapshot>, snap: &mut Snapshot) {
    let mut seen = SEEN.lock().unwrap();
    carry_ocr_ids(prev, snap, seen.entry(w).or_default());
}

/// OCR elements have no stable key, so `merge_ocr` numbers them afresh. Give each one, in
/// this order, the id of: the previous snapshot's OCR element with the same text; the one in
/// the same place that reads about the same (re-reads jitter: "Use the current in" for "Use
/// the current implied"); an earlier one (`seen`) with that text in that place. New ones get
/// ids above all used. `seen` then holds the ids handed out.
fn carry_ocr_ids(prev: Option<&Snapshot>, snap: &mut Snapshot, seen: &mut Vec<Seen>) {
    let max = |s: &Snapshot| s.elements.iter().map(|e| e.id).max().unwrap_or(0);
    let seen_max = seen.iter().map(|s| s.id).max().unwrap_or(0);
    let mut next = prev.map_or(0, max).max(max(snap)).max(seen_max);
    // Ids the element tree uses now cannot be handed to OCR text.
    let mut taken: Vec<u32> = snap
        .elements
        .iter()
        .filter(|e| e.kind != "ocr")
        .map(|e| e.id)
        .collect();
    let olds: Vec<(u32, &str, Rect)> = prev
        .into_iter()
        .flat_map(|p| &p.elements)
        .filter(|o| o.kind == "ocr")
        .map(|o| (o.id, o.name.as_str(), o.rect))
        .collect();
    let earlier: Vec<(u32, &str, Rect)> = seen
        .iter()
        .rev()
        .map(|s| (s.id, s.name.as_str(), s.rect))
        .collect();
    let overlaps = |a: &Rect, b: &Rect| a.intersect(b).area() * 2 > a.area().min(b.area());
    let mut given: Vec<(u32, String, Rect)> = Vec::new();
    for e in snap.elements.iter_mut().filter(|e| e.kind == "ocr") {
        let free = |c: &&(u32, &str, Rect)| !taken.contains(&c.0);
        let similar =
            |c: &&(u32, &str, Rect)| overlaps(&c.2, &e.rect) && desk::ocr_similar(c.1, &e.name);
        let old = olds
            .iter()
            .filter(free)
            .find(|c| c.1 == e.name)
            .or_else(|| olds.iter().filter(free).find(similar))
            .or_else(|| earlier.iter().filter(free).find(similar));
        e.id = match old {
            Some(c) => c.0,
            None => {
                next += 1;
                next
            }
        };
        taken.push(e.id);
        given.push((e.id, e.name.clone(), e.rect));
    }
    seen.retain(|s| !taken.contains(&s.id));
    seen.extend(
        given
            .into_iter()
            .map(|(id, name, rect)| Seen { id, name, rect }),
    );
    let extra = seen.len().saturating_sub(SEEN_MAX);
    seen.drain(..extra);
}

/// Before acting on an OCR element: re-capture, re-OCR its box and compare the text.
pub async fn check_ocr(
    w: WindowId,
    rect: Rect,
    want: String,
    ocr: OcrEngine,
) -> Result<(), String> {
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
    if desk::ocr_matches(&desk::ocr_norm(&got), &desk::ocr_norm(&want))
        || desk::ocr_similar(&got, &want)
    {
        Ok(())
    } else {
        tracing::warn!(want = %want, got = %got.trim(), "OCR element text changed");
        Err(format!("text there is now {:?}", got.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(lines: &[(&str, i32)]) -> Snapshot {
        let mut s = Snapshot {
            window: desk::WindowInfo {
                id: 1,
                title: String::new(),
                process: String::new(),
                pid: 1,
                rect: Rect::new(0, 0, 800, 600),
                monitor: 0,
                minimized: false,
                hidden: false,
                focused: false,
                maximized: false,
            },
            seq: 0,
            elements: Vec::new(),
        };
        let lines = lines
            .iter()
            .map(|&(t, y)| (t.to_string(), Rect::new(10, y, 300, 20)));
        desk::merge_ocr(&mut s, lines.collect());
        s
    }

    #[test]
    fn ocr_ids_follow_text_then_place() {
        let mut old = snap(&[("Send", 10), ("Use the current implied", 40)]);
        old.elements[0].id = 7;
        old.elements[1].id = 9;
        let mut new = snap(&[("Use the current in", 40), ("Brand new", 70), ("Send", 10)]);
        carry_ocr_ids(Some(&old), &mut new, &mut Vec::new());
        let ids: Vec<u32> = new.elements.iter().map(|e| e.id).collect();
        assert_eq!(ids, [9, 10, 7]);
    }

    #[test]
    fn a_reopened_menu_keeps_its_ids() {
        let mut seen = Vec::new();
        let mut open = snap(&[("File", 10), ("Show plots", 40), ("Export", 70)]);
        carry_ocr_ids(None, &mut open, &mut seen);
        let ids = |s: &Snapshot| s.elements.iter().map(|e| e.id).collect::<Vec<_>>();
        let first = ids(&open);
        let mut closed = snap(&[("File", 10)]);
        carry_ocr_ids(Some(&open), &mut closed, &mut seen);
        assert_eq!(ids(&closed), first[..1]);
        // Reopened, read a little differently.
        let mut again = snap(&[("File", 10), ("Show plots", 40), ("Exprt", 70)]);
        carry_ocr_ids(Some(&closed), &mut again, &mut seen);
        assert_eq!(ids(&again), first);
        // Same text somewhere else is not the old element.
        let mut moved = snap(&[("File", 10), ("Show plots", 300)]);
        carry_ocr_ids(Some(&closed), &mut moved, &mut seen);
        assert_ne!(ids(&moved)[1], first[1]);
    }

    #[test]
    fn menu_bar_titles_split_on_the_blank_gap() {
        let (w, h) = (200usize, 24usize);
        let mut img = Image {
            width: w as u32,
            height: h as u32,
            rgba: vec![255; w * h * 4],
            origin: (1000, 500),
        };
        // Ink for "File" 10..40, "View" 45..75 (a space), "Help" 110..140 (menu gap).
        for (x0, x1) in [(10, 40), (45, 75), (110, 140)] {
            for y in 6..18 {
                for x in x0..x1 {
                    img.rgba[(y * w + x) * 4..][..3].fill(0);
                }
            }
        }
        // A border along the bottom must not count as ink.
        img.rgba[(22 * w) * 4..(23 * w) * 4].fill(0);
        let r = |x: i32, w: i32| ywza_ocr::Rect {
            x: 1000 + x,
            y: 500,
            w,
            h: 24,
        };
        let word = |t: &str, x, w| ywza_ocr::Word {
            text: t.into(),
            rect: r(x, w),
        };
        let line = ywza_ocr::TextLine {
            text: "File View Help".into(),
            rect: r(0, 200),
            // CTC word boxes are rough: they need not line up with the ink.
            words: vec![
                word("File", 8, 28),
                word("View", 47, 25),
                word("Help", 105, 30),
            ],
        };
        let p = pieces(&line, &img);
        assert_eq!(p.len(), 2, "{p:?}");
        assert_eq!(p[0].0, "File View");
        assert_eq!(p[1], ("Help".into(), Rect::new(1110, 500, 30, 24)));
    }

    #[test]
    fn ocr_engine_choice() {
        assert_eq!(engine(OcrEngine::Windows, true), ywza_ocr::Engine::Paddle);
        assert_eq!(engine(OcrEngine::Auto, false), ywza_ocr::Engine::Auto);
    }
}
