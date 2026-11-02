use crate::platform::windows::util::hwnd;
use crate::platform::windows::wm;
use crate::{Error, Result, WindowId};
use ::windows::Win32::Foundation::POINT;
use ::windows::Win32::System::SystemInformation::GetTickCount;
use ::windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use ::windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

static IDLE_MS: AtomicU64 = AtomicU64::new(1000);
/// Give up taking the mouse if the user stays active this long.
const MAX_IDLE_WAIT: Duration = Duration::from_secs(10);

/// How long the user must be idle before real input is sent (default 1000 ms).
pub fn set_idle_before_input(ms: u64) {
    IDLE_MS.store(ms, Ordering::Relaxed);
}

/// Brings `window` forward (after the user has been idle), runs `act`, then puts the
/// cursor back. The previous foreground window is deliberately never restored: a
/// click often opens a menu or dialog, and handing focus back closes it, and the
/// caller cannot tell a UI-changing action from a harmless one. The target stays in
/// front so the follow-up snapshot, capture and input hit it.
pub(super) fn with_foreground<T>(
    window: Option<WindowId>,
    act: impl FnOnce() -> Result<T>,
) -> Result<T> {
    wait_idle()?;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };
    if let Some(id) = window {
        wm::bring_to_front(hwnd(id))?;
    }
    let result = act();
    // Let the app consume the input before the cursor moves away again.
    sleep(Duration::from_millis(100));
    unsafe {
        let _ = SetCursorPos(cursor.x, cursor.y);
    }
    result
}

pub(super) fn wait_idle() -> Result<()> {
    let need = IDLE_MS.load(Ordering::Relaxed);
    let start = Instant::now();
    loop {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        let idle = unsafe {
            GetLastInputInfo(&mut info).ok()?;
            GetTickCount().wrapping_sub(info.dwTime) as u64
        };
        if idle >= need {
            return Ok(());
        }
        if start.elapsed() > MAX_IDLE_WAIT {
            return Err(Error::UserBusy);
        }
        sleep(Duration::from_millis((need - idle).min(200)));
    }
}
