//! X11: monitors (RandR), windows (EWMH), capture (GetImage) and input (XTest).
//! One short-lived connection per call.

mod atoms;
mod capture;
mod conn;
mod facts;
mod focus;
mod foreground;
mod input;
mod keyboard;
mod list;
mod monitors;
mod mouse;
mod resize;

pub use capture::{capture_monitor, capture_window};
pub use conn::{os, X};
pub use focus::{close_window, focus_window};
pub use foreground::set_idle_before_input;
pub use keyboard::press_keys;
pub use list::{list_windows, window_info};
pub use monitors::{list_monitors, set_dpi_awareness};
pub use resize::move_window;

pub(crate) use keyboard::type_text;
pub(crate) use mouse::{real_click, wheel};
