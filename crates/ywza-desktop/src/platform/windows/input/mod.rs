//! Element actions: UIA patterns first, real input (foreground + SendInput) otherwise.
//! Real input leaves the target window in the foreground (see `foreground::with_foreground`).

mod clipboard;
mod foreground;
mod keyboard;
mod lock;
mod mouse;
mod raw;
mod text;

pub use foreground::set_idle_before_input;
pub use keyboard::press_keys;
pub use mouse::{click, click_at, expand, long_press, scroll};
pub use text::{live_text, type_text};

pub(crate) use raw::{absolute, key};
