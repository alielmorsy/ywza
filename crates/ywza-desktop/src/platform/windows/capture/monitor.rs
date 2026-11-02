use super::wgc::{grab, item};
use crate::platform::windows::wm;
use crate::{Image, Result};

/// Captures monitor `index` (as in `list_monitors`).
pub fn capture_monitor(index: usize) -> Result<Image> {
    let m = wm::monitor_handle(index)?;
    let rect = crate::list_monitors()?
        .get(index)
        .map(|m| m.rect)
        .unwrap_or_default();
    let img = grab(item(|i| unsafe { i.CreateForMonitor(m) })?)?;
    Ok(Image {
        origin: (rect.x, rect.y),
        ..img
    })
}
