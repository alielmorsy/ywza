//! Linux implementation (X11): EWMH windows, RandR monitors, XTest input, GetImage
//! capture and AT-SPI elements. Wayland sessions without XWayland return a clear
//! `Error::Os("unsupported: ...")` from every call that needs the display.

mod a11y;
mod keysym;
mod launch;
mod live_capture;
mod live_input;
mod roles;
mod wait;
mod x;

pub use a11y::{click, expand, long_press, scroll, snapshot, type_text, validate};
pub use launch::launch;
pub use live_capture::{CursorWatch, MonitorCapture};
pub use live_input::{live_key, live_mouse_button, live_mouse_move, live_wheel};
pub use x::{
    capture_monitor, capture_window, close_window, focus_window, list_monitors, list_windows,
    move_window, press_keys, set_dpi_awareness, set_idle_before_input,
};

pub(crate) use a11y::Handle;
