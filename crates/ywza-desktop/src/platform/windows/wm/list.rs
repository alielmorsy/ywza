use super::facts::{facts, is_app_window, is_hidden_main, is_popup, title};
use super::monitors::monitor_handles;
use crate::platform::windows::util::{hwnd, rect, window_id};
use crate::{Error, Rect, Result, WindowId, WindowInfo};
use ::windows::core::{BOOL, PWSTR};
use ::windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use ::windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use ::windows::Win32::Graphics::Gdi::{MonitorFromWindow, HMONITOR, MONITOR_DEFAULTTONEAREST};
use ::windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, GW_OWNER,
};

fn top_level() -> Result<Vec<HWND>> {
    unsafe extern "system" fn push(h: HWND, out: LPARAM) -> BOOL {
        unsafe { (*(out.0 as *mut Vec<HWND>)).push(h) };
        true.into()
    }
    let mut all: Vec<HWND> = Vec::new();
    unsafe { EnumWindows(Some(push), LPARAM(&mut all as *mut _ as isize))? };
    Ok(all)
}

fn pid_of(h: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

/// Top-level popup windows of `root`'s process that overlap it, e.g. an open menu
/// (class `#32768`, WinForms/XAML dropdown hosts). They are not UIA children of the root.
pub(crate) fn popup_windows(root: &WindowInfo) -> Vec<WindowId> {
    let Ok(all) = top_level() else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|&h| window_id(h) != root.id)
        .filter(|&h| pid_of(h) == root.pid)
        .filter(|&h| {
            let owned = unsafe { GetWindow(h, GW_OWNER) }.is_ok_and(|o| window_id(o) == root.id);
            is_popup(&facts(h), owned) && !visible_rect(h).intersect(&root.rect).is_empty()
        })
        .map(window_id)
        .collect()
}

/// App windows in z-order (top first), then the hidden main window (closed to the
/// tray) of each process that has no visible one, marked `hidden`.
pub fn list_windows() -> Result<Vec<WindowInfo>> {
    let all: Vec<_> = top_level()?
        .into_iter()
        .map(|h| (h, facts(h), pid_of(h)))
        .collect();
    let monitors = monitor_handles();
    let mut out: Vec<WindowInfo> = all
        .iter()
        .filter(|(_, f, _)| is_app_window(f))
        .map(|&(h, _, _)| info_with(h, &monitors))
        .collect();
    let mut covered: Vec<u32> = out.iter().map(|w| w.pid).collect();
    for (h, f, pid) in &all {
        if is_hidden_main(f) && !covered.contains(pid) {
            covered.push(*pid);
            out.push(info_with(*h, &monitors));
        }
    }
    Ok(out)
}

pub(crate) fn window_info(id: WindowId) -> Result<WindowInfo> {
    let h = hwnd(id);
    if !unsafe { IsWindow(Some(h)) }.as_bool() {
        return Err(Error::NotFound(format!("window w{id}")));
    }
    Ok(info_with(h, &monitor_handles()))
}

fn info_with(h: HWND, monitors: &[HMONITOR]) -> WindowInfo {
    let pid = pid_of(h);
    let m = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) };
    WindowInfo {
        id: window_id(h),
        title: title(h),
        process: process_name(pid),
        pid,
        rect: visible_rect(h),
        monitor: monitors.iter().position(|&x| x == m).unwrap_or(0),
        minimized: unsafe { IsIconic(h) }.as_bool(),
        hidden: !unsafe { IsWindowVisible(h) }.as_bool(),
    }
}

/// DWM extended frame bounds: what the user sees, without the invisible resize border.
pub(crate) fn visible_rect(h: HWND) -> Rect {
    let mut r = RECT::default();
    unsafe {
        if DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut _,
            size_of::<RECT>() as u32,
        )
        .is_err()
        {
            let _ = GetWindowRect(h, &mut r);
        }
    }
    rect(r)
}

pub(crate) fn process_name(pid: u32) -> String {
    unsafe {
        // Fails for elevated processes when we are not elevated.
        let Ok(p) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return "?".into();
        };
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(p, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
                .is_ok();
        let _ = CloseHandle(p);
        if !ok {
            return "?".into();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or(&path).to_string()
    }
}
