//! Monitors, top-level windows, focus, geometry and close.

mod facts;
mod focus;
mod list;
mod monitors;
mod resize;

pub use focus::{close_window, focus_window};
pub use list::list_windows;
pub use monitors::{list_monitors, set_dpi_awareness};
pub use resize::move_window;

pub(crate) use focus::{bring_to_front, owned_by};
pub(crate) use list::{popup_windows, process_name, visible_rect, window_info};
pub(crate) use monitors::monitor_handle;
