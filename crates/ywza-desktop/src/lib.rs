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
pub use image::{encode_png, pixelate, Image};
pub use keys::{parse_keys, Combo, Key};
pub use marks::{draw_box, draw_marks};
pub use model::{Button, CursorShape, Launched, Monitor, Tier, WindowId, WindowInfo};
pub use platform::{
    capture_monitor, capture_window, click, click_at, clipboard_text, close_window, expand,
    find_program, focus_window, launch, list_monitors, list_windows, long_press, move_window,
    press_keys, scroll, set_clipboard_text, set_dpi_awareness, set_idle_before_input, snapshot,
    type_text, validate,
};
pub use platform::{
    live_key, live_mouse_button, live_mouse_move, live_text, live_wheel, CursorWatch, FocusWatch,
    MonitorCapture,
};
pub use platform::wait_activity;
pub use settle::{wait_settled, Activity, SETTLE_TIMEOUT};

/// Environment that makes Qt export its UI tree over AT-SPI (it decides once, at start):
/// Linux apps started with it list real elements instead of OCR lines.
pub const A11Y_ENV: &[(&str, &str)] = &[
    ("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1"),
    ("QT_ACCESSIBILITY", "1"),
];
pub use snapshot::{
    diff, merge_ocr, needs_ocr, ocr_matches, ocr_norm, ocr_similar, render_diff, resolve,
    set_learned, Anchor, Candidate, Delta, Element, Patterns, Resolve, Snapshot, Step,
};
