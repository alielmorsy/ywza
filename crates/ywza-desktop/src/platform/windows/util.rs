use crate::{Rect, WindowId};
use ::windows::Win32::Foundation::{HWND, RECT};
use ::windows::Win32::System::Com::CoIncrementMTAUsage;
use std::sync::Once;

/// Keeps the process MTA alive so any thread that has not chosen an apartment can use
/// UIA and WinRT without `CoInitializeEx` (which would clash with a caller's STA).
pub(crate) fn com() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let _ = CoIncrementMTAUsage();
    });
}

pub(crate) fn hwnd(id: WindowId) -> HWND {
    HWND(id as usize as *mut _)
}

pub(crate) fn window_id(h: HWND) -> WindowId {
    h.0 as usize as WindowId
}

pub(crate) fn rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

pub(crate) fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}
