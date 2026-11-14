use super::ghost::takeover;
use super::lock::InputLock;
use super::message;
use super::raw::{mouse, send};
use crate::platform::windows::util::hwnd;
use crate::platform::windows::{uia, wm};
use crate::{Button, Error, Result, Snapshot, Tier, WindowId};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL,
};
use ::windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, WHEEL_DELTA};
use std::thread::sleep;
use std::time::Duration;

/// Clicks an element: Invoke/Toggle/SelectionItem/ExpandCollapse for a single left
/// click, then window messages for classic controls, else a real click at the
/// element's centre. `real` skips the first two tiers.
pub fn click(snap: &Snapshot, id: u32, button: Button, count: u32, real: bool) -> Result<Tier> {
    let window = snap.window.id;
    if let (false, Button::Left, 1) = (real, button, count) {
        let _lock = InputLock::window(window)?;
        let (e, el) = uia::live(snap, id)?;
        if let Some(el) = el {
            if uia::pattern_click(&el, &e.patterns) {
                return Ok(Tier::Pattern);
            }
            match message::click(&el) {
                Some(true) => return Ok(Tier::Message),
                Some(false) => {
                    return Err(Error::Os(format!(
                        "element {id} did not change after a background click; check it, then retry with real: true if needed"
                    )))
                }
                None => {}
            }
        }
    }
    // Real input: retake the locks in global-then-window order and revalidate.
    let _lock = InputLock::foreground(Some(window))?;
    let (e, _) = uia::live(snap, id)?;
    real_click(real, window, e.rect.center(), button, count, 0)?;
    Ok(Tier::Takeover)
}

/// Opens an element's dropdown/menu (ExpandCollapse) instead of its main action.
pub fn expand(snap: &Snapshot, id: u32) -> Result<Tier> {
    let _lock = InputLock::window(snap.window.id)?;
    let (e, el) = uia::live(snap, id)?;
    match el {
        Some(el) if e.patterns.expand && uia::expand(&el) => Ok(Tier::Pattern),
        Some(_) if e.patterns.expand => Err(Error::Os(format!("element {id} refused to expand"))),
        _ => Err(Error::NotFound(format!(
            "element {id} has no dropdown to expand; click it normally"
        ))),
    }
}

/// A real click at a screen point of `window` (brought to the front first).
pub fn click_at(window: WindowId, at: (i32, i32), button: Button, count: u32) -> Result<()> {
    let _lock = InputLock::foreground(Some(window))?;
    real_click(true, window, at, button, count, 0)
}

pub fn long_press(snap: &Snapshot, id: u32, ms: u64) -> Result<Tier> {
    let _lock = InputLock::foreground(Some(snap.window.id))?;
    let (e, _) = uia::live(snap, id)?;
    real_click(false, snap.window.id, e.rect.center(), Button::Left, 1, ms)?;
    Ok(Tier::Takeover)
}

/// Scrolls `dy` wheel notches down (negative = up) and `dx` right, via ScrollPattern
/// when the element has it (unless `real`), else the wheel over the element or window
/// centre.
pub fn scroll(snap: &Snapshot, id: Option<u32>, dx: i32, dy: i32, real: bool) -> Result<Tier> {
    let window = snap.window.id;
    if let (Some(id), false) = (id, real) {
        let _lock = InputLock::window(window)?;
        let (e, el) = uia::live(snap, id)?;
        if el.is_some_and(|el| e.patterns.scroll && uia::scroll_pattern(&el, dx, dy)) {
            return Ok(Tier::Pattern);
        }
    }
    let _lock = InputLock::foreground(Some(window))?;
    let at = match id {
        Some(id) => uia::live(snap, id)?.0.rect.center(),
        None => wm::visible_rect(hwnd(window)).center(),
    };
    takeover(real, window, || {
        warp(at)?;
        let mut inputs = Vec::new();
        if dy != 0 {
            inputs.push(mouse(MOUSEEVENTF_WHEEL, at, -dy * WHEEL_DELTA as i32));
        }
        if dx != 0 {
            inputs.push(mouse(MOUSEEVENTF_HWHEEL, at, dx * WHEEL_DELTA as i32));
        }
        send(&inputs)
    })?;
    Ok(Tier::Takeover)
}

fn real_click(
    real: bool,
    window: WindowId,
    at: (i32, i32),
    button: Button,
    count: u32,
    hold_ms: u64,
) -> Result<()> {
    let (down, up) = match button {
        Button::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        Button::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        Button::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    };
    takeover(real, window, || {
        warp(at)?;
        send(&[mouse(MOUSEEVENTF_MOVE, at, 0)])?;
        for _ in 0..count.max(1) {
            send(&[mouse(down, at, 0)])?;
            if hold_ms > 0 {
                sleep(Duration::from_millis(hold_ms));
            }
            send(&[mouse(up, at, 0)])?;
        }
        Ok(())
    })
}

/// Puts the cursor on the target first: modern apps hit-test against the real cursor.
pub(super) fn warp((x, y): (i32, i32)) -> Result<()> {
    unsafe { SetCursorPos(x, y)? };
    Ok(())
}
