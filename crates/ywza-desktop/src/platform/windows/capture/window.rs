use super::gdi::print_window;
use super::wgc::{grab, item};
use crate::platform::windows::util::hwnd;
use crate::platform::windows::wm;
use crate::{Error, Image, Result, WindowId};
use ::windows::Win32::UI::WindowsAndMessaging::IsIconic;

/// Captures a window's visible area. Minimized windows have no pixels: focus them first.
pub fn capture_window(id: WindowId) -> Result<Image> {
    let info = wm::window_info(id)?;
    let h = hwnd(id);
    if unsafe { IsIconic(h) }.as_bool() {
        return Err(Error::Os(format!("w{id} is minimized; focus it first")));
    }
    let wgc = item(|i| unsafe { i.CreateForWindow(h) }).and_then(grab);
    match wgc {
        Ok(img) => Ok(Image {
            origin: (info.rect.x, info.rect.y),
            ..img
        }),
        Err(_) => print_window(id),
    }
}
