use crate::platform::windows::util::{from_wide, rect};
use crate::{Error, Monitor, Result};
use ::windows::core::BOOL;
use ::windows::Win32::Foundation::{LPARAM, RECT};
use ::windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use ::windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetDpiForMonitor, GetThreadDpiAwarenessContext,
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};

pub fn set_dpi_awareness() -> Result<()> {
    unsafe {
        let pmv2 = DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;
        match SetProcessDpiAwarenessContext(pmv2) {
            Ok(()) => Ok(()),
            // Already set (manifest or earlier call): fine if it is what we want.
            Err(_)
                if AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), pmv2).as_bool() =>
            {
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}

pub(super) fn monitor_handles() -> Vec<HMONITOR> {
    unsafe extern "system" fn push(m: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        unsafe { (*(out.0 as *mut Vec<HMONITOR>)).push(m) };
        true.into()
    }
    let mut out: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(push), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

pub(crate) fn monitor_handle(index: usize) -> Result<HMONITOR> {
    monitor_handles()
        .get(index)
        .copied()
        .ok_or_else(|| Error::NotFound(format!("monitor {index}")))
}

pub fn list_monitors() -> Result<Vec<Monitor>> {
    monitor_handles()
        .into_iter()
        .enumerate()
        .map(|(index, m)| unsafe {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            GetMonitorInfoW(m, &mut info as *mut _ as *mut MONITORINFO).ok()?;
            let (mut dx, mut dy) = (96, 96);
            let _ = GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
            Ok(Monitor {
                index,
                name: from_wide(&info.szDevice),
                rect: rect(info.monitorInfo.rcMonitor),
                work: rect(info.monitorInfo.rcWork),
                scale: dx as f32 / 96.0,
                primary: info.monitorInfo.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
            })
        })
        .collect()
}
