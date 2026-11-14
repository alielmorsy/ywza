//! Window reads shared by every watcher: each window is re-read at most every few seconds,
//! and not at all while its UI events say nothing changed.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use ywza_desktop::{self as desk, Snapshot, WindowId};

use crate::tools::blocking;

const SNAP_GAP: Duration = Duration::from_millis(2500);
/// Re-read this often even when no event arrived (lost events).
const RESNAP_MAX: Duration = Duration::from_secs(10);
const OCR_GAP: Duration = Duration::from_secs(5);
/// A window nobody asked about for this long is forgotten.
const IDLE: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Shot {
    snap: Option<Arc<Snapshot>>,
    checked: Option<Instant>,
    read: Option<Instant>,
    /// Event count before the last read.
    count: u64,
    ocr: Option<(Instant, Option<Arc<Vec<String>>>)>,
    used: Option<Instant>,
}

#[derive(Default)]
pub struct Shots(HashMap<WindowId, Shot>);

impl Shots {
    /// The window's element list, up to a few seconds old. None: it could not be read.
    // shortcut: without UI events (Linux, silent apps) every read walks the whole tree; upgrade when that costs on big windows.
    pub async fn tree(&mut self, w: WindowId) -> Option<Arc<Snapshot>> {
        let e = self.0.entry(w).or_default();
        e.used = Some(Instant::now());
        if let (Some(s), Some(c)) = (&e.snap, e.checked) {
            if c.elapsed() < SNAP_GAP {
                return Some(s.clone());
            }
        }
        let since = if e.snap.is_some() { e.count } else { u64::MAX };
        let a = blocking(move || desk::wait_activity(w, since, Duration::ZERO)).await;
        e.checked = Some(Instant::now());
        if let Some(s) = &e.snap {
            if a.live && !a.changed && e.read.is_some_and(|r| r.elapsed() < RESNAP_MAX) {
                return Some(s.clone());
            }
        }
        let prev = e.snap.clone();
        match blocking(move || desk::snapshot(w, prev.as_deref())).await {
            Ok(s) => {
                let s = Arc::new(s);
                e.snap = Some(s.clone());
                e.count = a.count;
                e.read = Some(Instant::now());
                Some(s)
            }
            Err(_) => {
                self.0.remove(&w);
                None
            }
        }
    }

    /// The window's OCR lines, up to a few seconds old. None: OCR failed.
    pub async fn ocr(&mut self, w: WindowId, engine: ywza_ocr::Engine) -> Option<Arc<Vec<String>>> {
        let e = self.0.entry(w).or_default();
        e.used = Some(Instant::now());
        if let Some((at, lines)) = &e.ocr {
            if at.elapsed() < OCR_GAP {
                return lines.clone();
            }
        }
        let lines = blocking(move || read_lines(w, engine)).await.map(Arc::new);
        e.ocr = Some((Instant::now(), lines.clone()));
        lines
    }

    pub fn sweep(&mut self) {
        self.0
            .retain(|_, s| s.used.is_some_and(|u| u.elapsed() < IDLE));
    }
}

fn read_lines(w: WindowId, engine: ywza_ocr::Engine) -> Option<Vec<String>> {
    let img = desk::capture_window(w).ok()?;
    let img = ywza_ocr::Image {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    };
    let lines = ywza_ocr::recognize(&img, engine).ok()?;
    Some(lines.into_iter().map(|l| l.text).collect())
}
