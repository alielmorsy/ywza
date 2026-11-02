//! ywza-desktop: monitors, windows, UI elements, input and capture.
//!
//! All coordinates are physical virtual-screen pixels (call [`set_dpi_awareness`] first).
//! Platform code lives in `platform/*`; unsupported platforms return [`Error::Unsupported`].

mod error;
mod geometry;
mod image;
mod keys;
mod marks;
mod model;
mod platform;
mod settle;
mod snapshot;

pub use error::{Error, Result};
pub use geometry::Rect;
pub use image::{encode_png, Image};
pub use keys::{parse_keys, Combo, Key};
pub use marks::draw_marks;
pub use model::{Button, CursorShape, Launched, Monitor, WindowId, WindowInfo};
pub use platform::{
    capture_monitor, capture_window, click, close_window, expand, focus_window, launch,
    list_monitors, list_windows, long_press, move_window, press_keys, scroll, set_dpi_awareness,
    set_idle_before_input, snapshot, type_text, validate,
};
pub use platform::{
    live_key, live_mouse_button, live_mouse_move, live_wheel, CursorWatch, MonitorCapture,
};
pub use settle::{wait_settled, SETTLE_TIMEOUT};
pub use snapshot::{diff, merge_ocr, needs_ocr, Element, Patterns, Snapshot};
