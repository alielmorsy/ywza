//! Linux implementation (X11): EWMH windows, RandR monitors, XTest input, GetImage
//! capture and AT-SPI elements. Wayland sessions without XWayland return a clear
//! `Error::Os("unsupported: ...")` from every call that needs the display.

use crate::{settle::Activity, WindowId};
use std::time::Duration;

mod a11y;
mod clipboard;
mod keysym;
mod launch;
mod live_capture;
mod live_input;
mod roles;
mod wait;
mod x;

pub use a11y::{click, expand, long_press, scroll, snapshot, type_text, validate};
pub use clipboard::{clipboard_text, set_clipboard_text};
pub use launch::{find_program, launch};
pub use live_capture::{CursorWatch, FocusWatch, MonitorCapture};
pub use live_input::{live_key, live_mouse_button, live_mouse_move, live_text, live_wheel};
pub use x::{
    capture_monitor, capture_window, close_window, focus_window, list_monitors, list_windows,
    move_window, press_keys, set_dpi_awareness, set_idle_before_input,
};

pub(crate) use a11y::Handle;

/// A real click at a screen point of `window` (brought to the front first).
pub fn click_at(
    window: crate::WindowId,
    at: (i32, i32),
    button: crate::Button,
    count: u32,
) -> crate::Result<()> {
    x::real_click(window, at, button, count, 0)
}

// shortcut: AT-SPI events not wired, Linux settles and watches by polling; upgrade when polling cost shows up on big windows
pub fn wait_activity(_window: WindowId, _since: u64, _max: Duration) -> Activity {
    Activity {
        count: 0,
        changed: false,
        live: false,
    }
}

pub fn mark_silent(_window: WindowId) {}
