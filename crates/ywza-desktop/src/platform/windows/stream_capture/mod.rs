//! Live view support: continuous monitor capture, cursor shape and raw human input
//! (no idle wait, no foreground juggling: the human drives the mouse directly).

mod cursor;
mod focus;
mod live_input;
mod monitor;

pub use cursor::CursorWatch;
pub use focus::FocusWatch;
pub use live_input::{live_key, live_mouse_button, live_mouse_move, live_wheel};
pub use monitor::MonitorCapture;
