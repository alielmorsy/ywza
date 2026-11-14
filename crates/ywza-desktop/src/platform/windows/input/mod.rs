//! Element actions, least intrusive first: UIA patterns, window messages to classic
//! controls (`message`), then real input (foreground + SendInput). Ghost mode's real
//! input gives the desktop back where safe (`ghost`); with `real` set the target stays in
//! the foreground (see `foreground::with_foreground`).

mod clipboard;
mod foreground;
mod ghost;
mod keyboard;
mod lock;
mod message;
mod mouse;
mod raw;
mod text;

pub use clipboard::{set_text as set_clipboard_text, text as clipboard_text};
pub use foreground::set_idle_before_input;
pub use keyboard::press_keys;
pub use mouse::{click, click_at, expand, long_press, scroll};
pub use text::{live_text, type_text};

pub(crate) use raw::{absolute, key};
