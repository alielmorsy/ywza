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
    capture_monitor, capture_window, click, click_at, close_window, expand, focus_window, launch,
    list_monitors, list_windows, long_press, move_window, press_keys, scroll, set_dpi_awareness,
    set_idle_before_input, snapshot, type_text, validate,
};
pub use platform::{
    live_key, live_mouse_button, live_mouse_move, live_wheel, CursorWatch, MonitorCapture,
};
pub use settle::{wait_settled, SETTLE_TIMEOUT};

/// Environment that makes Qt export its UI tree over AT-SPI (it decides once, at start):
/// Linux apps started with it list real elements instead of OCR lines.
pub const A11Y_ENV: &[(&str, &str)] = &[
    ("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1"),
    ("QT_ACCESSIBILITY", "1"),
];
pub use snapshot::{
    diff, merge_ocr, needs_ocr, ocr_matches, ocr_norm, ocr_similar, Element, Patterns, Snapshot,
};
