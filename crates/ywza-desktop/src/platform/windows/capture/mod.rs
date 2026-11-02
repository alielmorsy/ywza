//! Single-frame capture via Windows.Graphics.Capture, with PrintWindow as fallback.

mod gdi;
mod monitor;
mod pixels;
mod wgc;
mod window;

pub use monitor::capture_monitor;
pub use window::capture_window;
