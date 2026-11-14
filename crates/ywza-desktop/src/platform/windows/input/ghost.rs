//! Ghost mode's real-input fallback: a brief takeover that leaves the desktop as it
//! was whenever that is safe. Same locks and idle wait as `with_foreground`; on top it
//! blocks the user's own input while it runs (needs elevation, otherwise skipped) and
//! gives the foreground back to the previous window.

use super::foreground::{wait_idle, with_foreground};
use crate::platform::windows::util::{hwnd, window_id};
use crate::platform::windows::wm;
use crate::{Result, WindowId};
use ::windows::Win32::Foundation::{HWND, POINT};
use ::windows::Win32::UI::Input::KeyboardAndMouse::BlockInput;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SetCursorPos,
    GUITHREADINFO, GUI_INMENUMODE, GUI_POPUPMENUMODE,
};
use std::thread::sleep;
use std::time::Duration;

/// Runs `act` with real input on `window`. `real` keeps the plain takeover (the window
/// stays in front); otherwise the ghost one.
pub(super) fn takeover<T>(
    real: bool,
    window: WindowId,
    act: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if real {
        return with_foreground(Some(window), act);
    }
    wait_idle()?;
    let prev = unsafe { GetForegroundWindow() };
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };
    let target = hwnd(window);
    let _block = Block::on();
    if !wm::owned_by(prev, target) {
        wm::bring_to_front(target)?;
    }
    let result = act();
    // Let the app consume the input before the cursor moves away again.
    sleep(Duration::from_millis(100));
    unsafe {
        let _ = SetCursorPos(cursor.x, cursor.y);
    }
    give_back(prev, target);
    result
}

/// Puts `prev` back in front when that cannot undo what the action did.
pub(super) fn give_back(prev: HWND, target: HWND) {
    let foreign = !prev.0.is_null() && prev != target && !wm::owned_by(prev, target);
    let stays = unsafe { GetForegroundWindow() } == target && !opened_popup(target);
    if restores(foreign, stays) {
        let _ = wm::bring_to_front(prev);
    }
}

/// `foreign`: the previous foreground was neither the target nor one of its dialogs.
/// `stays`: the target still holds the foreground and nothing popped up over it.
fn restores(foreign: bool, stays: bool) -> bool {
    foreign && stays
}

/// A menu is open on the target's thread, or its app has a popup or dialog up.
fn opened_popup(target: HWND) -> bool {
    let thread = unsafe { GetWindowThreadProcessId(target, None) };
    let mut gui = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    let menu = unsafe { GetGUIThreadInfo(thread, &mut gui) }
        .is_ok_and(|_| gui.flags.0 & (GUI_INMENUMODE.0 | GUI_POPUPMENUMODE.0) != 0);
    menu || wm::window_info(window_id(target)).is_ok_and(|w| !wm::popup_windows(&w).is_empty())
}

/// Blocks the user's keyboard and mouse while it lives; a no-op without elevation.
pub(super) struct Block(bool);

impl Block {
    pub(super) fn on() -> Block {
        Block(unsafe { BlockInput(true) }.is_ok())
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        if self.0 {
            let _ = unsafe { BlockInput(false) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreground_returns_only_when_nothing_opened() {
        assert!(restores(true, true));
        assert!(!restores(false, true)); // it was the target or its dialog
        assert!(!restores(true, false)); // a menu, popup or new window is up
    }
}
