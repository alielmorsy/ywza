//! Teach by touch: the user's clicks while the run waits become plan steps.

use std::time::Duration;

use ywza_desktop::{Element, Snapshot};

use super::Sos;
use crate::{
    anchor::SavedAnchor,
    plans::{PlanStep, WindowRef},
};

/// Lets the UI react to a tap before the snapshot that later taps map against.
const SETTLE: Duration = Duration::from_millis(250);

impl Sos {
    /// A left click at virtual-screen pixels (x, y): recorded in every paused session whose
    /// window holds it, as a click on the smallest element there. Typed text and keys are never
    /// recorded, they may be secrets.
    /// shortcut: taps map against the window's last snapshot, not the UI at the instant of the
    /// tap; a fast second tap can hit a stale layout. Upgrade by snapshotting on pointer down.
    pub fn touch(&self, x: i32, y: i32) {
        let mut paused = self.paused.lock().unwrap();
        for (sid, p) in paused.iter_mut() {
            let Some(snap) = p.snap.as_ref().filter(|s| inside(s.window.rect, x, y)) else {
                continue;
            };
            let Some(el) = smallest(snap, x, y) else {
                continue;
            };
            p.steps.push(PlanStep {
                tool: "click".into(),
                window: Some(WindowRef {
                    process: snap.window.process.clone(),
                    title: snap.window.title.clone(),
                }),
                anchor: Some(SavedAnchor::from(&el.anchor)),
                expect: (!el.name.is_empty()).then(|| el.name.clone()),
                ..Default::default()
            });
            let (prev, sid, map) = (snap.clone(), *sid, self.paused.clone());
            tokio::task::spawn_blocking(move || {
                std::thread::sleep(SETTLE);
                if let Ok(fresh) = ywza_desktop::snapshot(prev.window.id, Some(&prev)) {
                    if let Some(p) = map.lock().unwrap().get_mut(&sid) {
                        p.snap = Some(fresh);
                    }
                }
            });
        }
    }
}

fn inside(r: ywza_desktop::Rect, x: i32, y: i32) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h
}

fn smallest(snap: &Snapshot, x: i32, y: i32) -> Option<&Element> {
    snap.elements
        .iter()
        .filter(|e| !matches!(e.kind, "text" | "image") && inside(e.rect, x, y))
        .min_by_key(|e| e.rect.area())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ywza_desktop::Rect;

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect::new(10, 10, 5, 5);
        assert!(inside(r, 10, 10) && inside(r, 14, 14));
        assert!(!inside(r, 15, 10) && !inside(r, 9, 12));
        assert!(!inside(Rect::new(0, 0, 0, 5), 0, 0));
    }
}
