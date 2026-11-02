use super::raw::{automation, cache_request, raw_from};
use crate::platform::windows::util::hwnd;
use crate::platform::windows::wm;
use crate::snapshot::recheck;
use crate::{Element, Error, Rect, Result, Snapshot};
use ::windows::Win32::UI::Accessibility::IUIAutomationElement;
use ::windows::Win32::UI::WindowsAndMessaging::IsWindow;

/// Re-resolves an element and checks it still matches its snapshot entry.
/// OCR elements only check that the window exists; the caller re-OCRs the box.
pub fn validate(snap: &Snapshot, id: u32) -> Result<()> {
    live(snap, id).map(|_| ())
}

/// Validates and returns the element with its fresh UIA object (None for OCR).
pub(crate) fn live(snap: &Snapshot, id: u32) -> Result<(&Element, Option<IUIAutomationElement>)> {
    let stale = |reason: &str| Error::Stale {
        id,
        reason: reason.into(),
    };
    let e = snap.get(id).ok_or_else(|| stale("unknown id"))?;
    let h = hwnd(snap.window.id);
    if !unsafe { IsWindow(Some(h)) }.as_bool() {
        return Err(stale("window closed"));
    }
    let Some(handle) = &e.handle else {
        return Ok((e, None));
    };
    let uia = automation()?;
    let fresh =
        unsafe { handle.0.BuildUpdatedCache(&cache_request(&uia)?) }.map_err(|_| stale("gone"))?;
    let mut raw = raw_from(&fresh).map_err(|_| stale("gone"))?;
    if raw.key != e.key {
        return Err(stale("replaced"));
    }
    // Popups may sit outside the window and need not move with it: compare on screen.
    let screen = Rect::new(-1_000_000, -1_000_000, 2_000_000, 2_000_000);
    raw.bounds = e.popup.then_some(screen);
    let (old_win, win) = if e.popup {
        (screen, screen)
    } else {
        (snap.window.rect, wm::visible_rect(h))
    };
    recheck(e, &raw, &old_win, &win).map_err(|r| stale(&r))?;
    Ok((e, Some(fresh)))
}
