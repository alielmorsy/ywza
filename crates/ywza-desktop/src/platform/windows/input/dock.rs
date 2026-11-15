//! The ywza dock is always on top near the top edge; real input aimed at a point under it
//! would hit the dock instead of the target, so it steps aside for the input.

use ::windows::Win32::Foundation::{HWND, POINT};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetWindowTextW, ShowWindow, WindowFromPoint, GA_ROOT, SW_HIDE,
    SW_SHOWNOACTIVATE,
};

const TITLE: &str = "ywza dock";

/// Shows the dock again when dropped.
pub(super) struct DockAside(Option<HWND>);

/// Hides the dock while the guard lives if it is the window at `at`.
pub(super) fn aside((x, y): (i32, i32)) -> DockAside {
    let root = unsafe { GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT) };
    if root.is_invalid() {
        return DockAside(None);
    }
    let mut buf = [0u16; 32];
    let n = unsafe { GetWindowTextW(root, &mut buf) }.max(0) as usize;
    if String::from_utf16_lossy(&buf[..n]) != TITLE {
        return DockAside(None);
    }
    let _ = unsafe { ShowWindow(root, SW_HIDE) };
    DockAside(Some(root))
}

impl Drop for DockAside {
    fn drop(&mut self) {
        if let Some(h) = self.0 {
            let _ = unsafe { ShowWindow(h, SW_SHOWNOACTIVATE) };
        }
    }
}
