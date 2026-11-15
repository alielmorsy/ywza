//! The window that was in front when the shortcut fired, so the dock can hand focus back to it.

#[cfg(windows)]
pub use win::{current, ignore, remember, restore};

#[cfg(not(windows))]
pub fn current() -> Option<(String, String, String)> {
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
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, SetForegroundWindow,
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

    /// With one of our own windows in front, the previous target stays.
    pub fn remember() {
        let h = unsafe { GetForegroundWindow() }.0 as isize;
        if OWN.lock().unwrap().contains(&h) {
            return;
        }
        *TARGET.lock().unwrap() = (h != 0).then_some(h);
    }

    /// `(w<id>, title, exe file name)` of the remembered window.
    pub fn current() -> Option<(String, String, String)> {
        let h = (*TARGET.lock().unwrap())?;
        let mut buf = [0u16; 512];
        let n = unsafe { GetWindowTextW(hwnd(h), &mut buf) }.max(0) as usize;
        Some((
            format!("w{h}"),
            String::from_utf16_lossy(&buf[..n]),
            process(hwnd(h)),
        ))
    }

    fn process(h: HWND) -> String {
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid as *mut u32));
            // Fails for elevated processes when we are not elevated.
            let Ok(p) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return "?".into();
            };
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                p,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
            .is_ok();
            let _ = CloseHandle(p);
            if !ok {
                return "?".into();
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            path.rsplit('\\').next().unwrap_or(&path).to_string()
        }
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
