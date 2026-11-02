//! Live view on X11: paced monitor capture (MIT-SHM when available) with the cursor
//! composited in, returning only frames that changed.

mod cursor;
mod monitor;
mod shm;

pub use cursor::CursorWatch;
pub use monitor::MonitorCapture;
