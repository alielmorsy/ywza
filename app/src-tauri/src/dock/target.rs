//! The window that was in front when the shortcut fired, so the dock can hand focus back to it.

#[cfg(windows)]
pub use win::{current, ignore, remember, restore};

#[cfg(not(windows))]
pub fn current() -> Option<(String, String)> {
    None
}

#[cfg(not(windows))]
pub fn restore() -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::Mutex;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, SetForegroundWindow,
    };

    static TARGET: Mutex<Option<isize>> = Mutex::new(None);
    static OWN: Mutex<Vec<isize>> = Mutex::new(Vec::new());

    fn hwnd(v: isize) -> HWND {
        HWND(v as *mut c_void)
    }

    /// Our own windows are never a target.
    pub fn ignore(handles: Vec<isize>) {
        *OWN.lock().unwrap() = handles;
    }

    pub fn remember() {
        let h = unsafe { GetForegroundWindow() }.0 as isize;
        let own = OWN.lock().unwrap().contains(&h);
        *TARGET.lock().unwrap() = (h != 0 && !own).then_some(h);
    }

    pub fn current() -> Option<(String, String)> {
        let h = (*TARGET.lock().unwrap())?;
        let mut buf = [0u16; 512];
        let n = unsafe { GetWindowTextW(hwnd(h), &mut buf) }.max(0) as usize;
        Some((format!("w{h}"), String::from_utf16_lossy(&buf[..n])))
    }

    pub fn restore() -> Result<(), String> {
        let Some(h) = *TARGET.lock().unwrap() else {
            return Ok(());
        };
        if unsafe { SetForegroundWindow(hwnd(h)) }.as_bool() {
            Ok(())
        } else {
            Err("Could not restore the previous window".to_string())
        }
    }
}
