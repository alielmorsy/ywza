//! UI Automation: snapshots via a cached control-view walk, validation, pattern actions.

mod chromium;
mod events;
mod handle;
mod live;
mod patterns;
mod raw;
mod role;
mod variant;
mod walk;

pub use events::{mark_silent, wait_activity};
pub use handle::Handle;
pub use live::validate;
pub use walk::snapshot;

pub(crate) use handle::Uia;
pub(crate) use live::live;
pub(crate) use patterns::{
    expand, focused, has_focus, is_web, norm, pattern_click, render_widget, scroll_pattern,
    set_focus, set_value, text, value_is,
};
