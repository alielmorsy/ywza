use super::list::{visible_rect, window_info};
use super::monitors::list_monitors;
use crate::platform::windows::util::hwnd;
use crate::{Error, Rect, Result, WindowId};
use ::windows::Win32::Foundation::RECT;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, IsIconic, IsZoomed, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER,
    SW_RESTORE,
};

/// Moves/resizes using visible bounds. `x`/`y` are relative to the target monitor's
/// work area; missing values keep the current position (relative to its monitor) and size.
pub fn move_window(
    id: WindowId,
    monitor: Option<usize>,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<i32>,
    h: Option<i32>,
) -> Result<()> {
    let info = window_info(id)?;
    let hw = hwnd(id);
    unsafe {
        if IsZoomed(hw).as_bool() || IsIconic(hw).as_bool() {
            let _ = ShowWindow(hw, SW_RESTORE);
        }
    }
    let monitors = list_monitors()?;
    let cur = monitors
        .get(info.monitor)
        .ok_or_else(|| Error::NotFound("current monitor".into()))?;
    let target = match monitor {
        Some(i) => monitors
            .get(i)
            .ok_or_else(|| Error::NotFound(format!("monitor {i}")))?,
        None => cur,
    };
    let vis = visible_rect(hw);
    let want = Rect::new(
        target.work.x + x.unwrap_or(vis.x - cur.work.x),
        target.work.y + y.unwrap_or(vis.y - cur.work.y),
        w.unwrap_or(vis.w),
        h.unwrap_or(vis.h),
    );
    // Second pass corrects the frame offset after a DPI change on another monitor.
    for _ in 0..2 {
        let vis = visible_rect(hw);
        if vis == want {
            break;
        }
        let mut wr = RECT::default();
        unsafe { GetWindowRect(hw, &mut wr)? };
        let (l, t) = (vis.x - wr.left, vis.y - wr.top);
        let (r, b) = (wr.right - (vis.x + vis.w), wr.bottom - (vis.y + vis.h));
        unsafe {
            SetWindowPos(
                hw,
                None,
                want.x - l,
                want.y - t,
                want.w + l + r,
                want.h + t + b,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )?;
        }
    }
    Ok(())
}
