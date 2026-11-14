//! Windows implementation: Win32 windows, UI Automation, SendInput, Windows.Graphics.Capture.

mod capture;
mod d3d;
mod error;
mod input;
mod launch;
mod stream_capture;
mod uia;
mod util;
mod wm;

pub use capture::{capture_monitor, capture_window};
pub use input::{
    click, click_at, clipboard_text, expand, live_text, long_press, press_keys, scroll,
    set_clipboard_text, set_idle_before_input, type_text,
};
pub use launch::{find_program, launch};
pub use stream_capture::{
    live_key, live_mouse_button, live_mouse_move, live_wheel, CursorWatch, FocusWatch,
    MonitorCapture,
};
pub use uia::{mark_silent, snapshot, validate, wait_activity};
pub use wm::{
    close_window, focus_window, list_monitors, list_windows, move_window, set_dpi_awareness,
};

pub(crate) use uia::Handle;
