use super::list::window_info;
use crate::platform::windows::util::{hwnd, window_id};
use crate::{Error, Result, WindowId};
use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use ::windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use ::windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    PostMessageW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW, WM_CLOSE,
};
use std::time::{Duration, Instant};

pub fn focus_window(id: WindowId) -> Result<()> {
    window_info(id)?;
    bring_to_front(hwnd(id))
}

/// Makes `h` the foreground window, showing it if hidden (closed to the tray) and
/// restoring it if minimized.
pub(crate) fn bring_to_front(h: HWND) -> Result<()> {
    unsafe {
        if !IsWindowVisible(h).as_bool() {
            let _ = ShowWindow(h, SW_SHOW);
        }
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        if GetForegroundWindow() == h {
            return Ok(());
        }
        // Windows only lets the foreground thread hand out focus; borrow its input state.
        let fg_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let me = GetCurrentThreadId();
        let attached =
            fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = BringWindowToTop(h);
        let _ = SetForegroundWindow(h);
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while GetForegroundWindow() != h {
            if Instant::now() > deadline {
                return Err(Error::Os(format!(
                    "could not bring w{} to the front",
                    window_id(h)
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    Ok(())
}

pub fn close_window(id: WindowId) -> Result<()> {
    window_info(id)?;
    unsafe { PostMessageW(Some(hwnd(id)), WM_CLOSE, WPARAM(0), LPARAM(0))? };
    Ok(())
}
