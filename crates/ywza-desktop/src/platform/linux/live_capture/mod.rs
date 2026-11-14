//! Live view on X11: paced monitor capture (MIT-SHM when available) with the cursor
//! composited in, returning only frames that changed.

mod cursor;
mod monitor;
mod shm;

pub use cursor::CursorWatch;
pub use monitor::MonitorCapture;

/// Where typed text would land. X11 has no caret API and AT-SPI only reports focus as
/// events, so this never knows; the phone's keyboard button still works.
// shortcut: always None on Linux; subscribe to AT-SPI focus events if Linux users need auto-detection.
pub struct FocusWatch;

impl FocusWatch {
    pub fn new() -> crate::Result<Self> {
        Ok(Self)
    }

    pub fn now(&self) -> crate::Result<Option<crate::Rect>> {
        Ok(None)
    }
}
