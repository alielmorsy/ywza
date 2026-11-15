//! The lens: small click-through marks over the target of the action about to run.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::{platform, Rect, WindowId};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// Outline around an element.
    Element,
    /// Ripple at a click point.
    Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Acting,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MarkId(pub u64);

static ENABLED: AtomicBool = AtomicBool::new(true);
static DWELL_MS: AtomicU64 = AtomicU64::new(120);

pub(crate) const FADE_IN_MS: u64 = 120;
const FADE_OUT_MS: u64 = 400;
const DONE_HOLD_MS: u64 = 250;
const FAILED_HOLD_MS: u64 = 1200;
const TIMEOUT_MS: u64 = 5000;

pub fn set_lens(enabled: bool, dwell_ms: u64) {
    DWELL_MS.store(dwell_ms, Ordering::Relaxed);
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// How long to wait between showing a mark and acting; None when the lens is off.
pub fn lens_dwell() -> Option<Duration> {
    ENABLED
        .load(Ordering::Relaxed)
        .then(|| Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)))
}

/// Shows a mark over `rect` of `window`. None when the lens is off, the overlay is
/// unavailable or the target is covered (then skip the dwell too).
/// For `Point`, `rect` is a 1x1 rect at the click point.
pub fn lens_mark(window: WindowId, rect: Rect, caption: &str, kind: MarkKind) -> Option<MarkId> {
    if !ENABLED.load(Ordering::Relaxed) || rect.is_empty() {
        return None;
    }
    platform::overlay_mark(window, rect, caption, kind)
}

pub fn lens_update(id: MarkId, phase: Phase) {
    platform::overlay_update(id, phase);
}

/// Runs `f` with every mark hidden, so the agent's own captures never see them.
pub fn lens_hidden<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            if self.0 {
                platform::overlay_restore();
            }
        }
    }
    let _restore = Restore(platform::overlay_hide());
    f()
}

/// Mark window bounds: `rect` grown by 8*scale on each side plus a 28*scale caption
/// row, above the rect when `top_room`, below it otherwise.
pub(crate) fn mark_bounds(rect: Rect, scale: f32, top_room: bool) -> Rect {
    let pad = px(8.0, scale);
    let room = px(28.0, scale);
    let y = if top_room {
        rect.y - pad - room
    } else {
        rect.y - pad
    };
    Rect::new(rect.x - pad, y, rect.w + 2 * pad, rect.h + 2 * pad + room)
}

/// Where a top-level window sits: its GA_ROOT, its GA_ROOTOWNER and its process.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Owner {
    pub root: WindowId,
    pub root_owner: WindowId,
    pub pid: u32,
}

/// A target is visible when the window under its center is the target's window, one
/// of its owned popups, or another window of its process (snapshots merge those).
pub(crate) fn visible(target: Owner, hit: Owner) -> bool {
    target.root != 0
        && (target.root == hit.root
            || target.root_owner == hit.root_owner
            || (target.pid != 0 && target.pid == hit.pid))
}

/// Mark opacity `elapsed_ms` after it was shown (`Acting`) or after its final phase.
pub(crate) fn alpha_at(phase: Phase, elapsed_ms: u64) -> u8 {
    match phase {
        Phase::Acting if elapsed_ms < FADE_IN_MS => (255 * elapsed_ms / FADE_IN_MS) as u8,
        Phase::Acting => fade(elapsed_ms, TIMEOUT_MS),
        Phase::Done => fade(elapsed_ms, DONE_HOLD_MS),
        Phase::Failed => fade(elapsed_ms, FAILED_HOLD_MS),
    }
}

fn fade(t: u64, hold: u64) -> u8 {
    let gone = t.saturating_sub(hold);
    (255 * FADE_OUT_MS.saturating_sub(gone) / FADE_OUT_MS) as u8
}

pub(crate) fn px(v: f32, scale: f32) -> i32 {
    (v * scale).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    // One function: both touch the global config and must not race.
    #[test]
    fn config_gates_marks_and_dwell() {
        set_lens(false, 120);
        assert!(lens_dwell().is_none());
        assert!(lens_mark(1, Rect::new(0, 0, 10, 10), "Click", MarkKind::Element).is_none());
        set_lens(true, 120);
        assert_eq!(lens_dwell(), Some(Duration::from_millis(120)));
    }

    #[test]
    fn covered_target_gets_no_mark() {
        let w = |root, root_owner, pid| Owner {
            root,
            root_owner,
            pid,
        };
        assert!(visible(w(5, 5, 1), w(5, 5, 1)));
        assert!(!visible(w(5, 5, 1), w(9, 9, 2)));
        assert!(visible(w(5, 5, 1), w(7, 5, 2))); // owned dialog
        assert!(visible(w(5, 5, 1), w(8, 8, 1))); // same-process menu or flyout
        assert!(!visible(w(0, 0, 0), w(0, 0, 0)));
    }

    #[test]
    fn mark_bounds_scale_with_dpi() {
        let a = mark_bounds(Rect::new(100, 200, 50, 20), 1.0, true);
        let b = mark_bounds(Rect::new(100, 200, 50, 20), 2.0, true);
        assert!(b.w - 50 == 2 * (a.w - 50));
        assert!(a.x == 92 && a.y < 200);
        let below = mark_bounds(Rect::new(100, 200, 50, 20), 1.0, false);
        assert_eq!(below.y, 192);
        assert_eq!(below.h, a.h);
    }

    #[test]
    fn marks_fade_out() {
        assert_eq!(alpha_at(Phase::Done, 0), 255);
        assert_eq!(alpha_at(Phase::Done, 2000), 0);
        assert_eq!(alpha_at(Phase::Failed, 1000), 255);
        assert_eq!(alpha_at(Phase::Acting, 0), 0);
        assert_eq!(alpha_at(Phase::Acting, 1000), 255);
        assert_eq!(alpha_at(Phase::Acting, 6000), 0);
    }
}
