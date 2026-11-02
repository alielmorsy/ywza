use super::bus::{block, connect, proxy};
use super::node::raw_of;
use crate::platform::linux::x;
use crate::snapshot::recheck;
use crate::{Element, Error, Rect, Result, Snapshot};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::zbus;

/// Re-resolves an element and checks it still matches its snapshot entry.
/// OCR elements only check that the window exists; the caller re-OCRs the box.
pub fn validate(snap: &Snapshot, id: u32) -> Result<()> {
    block(async { live(&connect().await?, snap, id).await.map(|_| ()) })
}

/// Validates and returns the element with its fresh proxy (None for OCR).
pub(super) async fn live<'a>(
    conn: &zbus::Connection,
    snap: &'a Snapshot,
    id: u32,
) -> Result<(&'a Element, Option<AccessibleProxy<'static>>)> {
    let stale = |reason: &str| Error::Stale {
        id,
        reason: reason.into(),
    };
    let e = snap.get(id).ok_or_else(|| stale("unknown id"))?;
    let win = x::window_info(snap.window.id).map_err(|_| stale("window closed"))?;
    let Some(h) = &e.handle else {
        return Ok((e, None));
    };
    let acc = proxy(conn, &h.bus, &h.path)
        .await
        .map_err(|_| stale("gone"))?;
    let (mut raw, _) = raw_of(&acc).await.map_err(|_| stale("gone"))?;
    if raw.key != e.key {
        return Err(stale("replaced"));
    }
    // Popups may sit outside the window and need not move with it: compare on screen.
    let screen = Rect::new(-1_000_000, -1_000_000, 2_000_000, 2_000_000);
    raw.bounds = e.popup.then_some(screen);
    let (old_win, now_win) = if e.popup {
        (screen, screen)
    } else {
        (snap.window.rect, win.rect)
    };
    recheck(e, &raw, &old_win, &now_win).map_err(|r| stale(&r))?;
    Ok((e, Some(acc)))
}
