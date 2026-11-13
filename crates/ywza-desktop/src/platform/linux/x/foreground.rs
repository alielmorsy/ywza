use super::conn::X;
use crate::{Error, Result, WindowId};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::protocol::xtest;

static IDLE_MS: AtomicU64 = AtomicU64::new(1000);
/// Give up taking the mouse if the user stays active this long.
const MAX_IDLE_WAIT: Duration = Duration::from_secs(10);

/// How long the user must be idle before real input is sent (default 1000 ms).
pub fn set_idle_before_input(ms: u64) {
    IDLE_MS.store(ms, Ordering::Relaxed);
}

/// Waits until the user has been idle long enough; `UserBusy` after [`MAX_IDLE_WAIT`].
fn wait_idle(x: &X) -> Result<()> {
    let need = IDLE_MS.load(Ordering::Relaxed);
    let start = Instant::now();
    loop {
        let idle = x.idle_ms();
        if idle >= need {
            return Ok(());
        }
        if start.elapsed() > MAX_IDLE_WAIT {
            return Err(Error::UserBusy);
        }
        sleep(Duration::from_millis((need - idle).min(200)));
    }
}

/// Brings `window` forward (after the user has been idle), runs `act`, then puts the
/// pointer back. Like on Windows the previous active window is deliberately not restored,
/// and an active dialog of the window (WM_TRANSIENT_FOR chain) keeps the focus and the input.
pub(super) fn with_foreground<T>(
    window: Option<WindowId>,
    act: impl FnOnce(&X) -> Result<T>,
) -> Result<T> {
    let x = X::connect()?;
    xtest::get_version(&x.conn, 2, 2)?
        .reply()
        .map_err(|e| Error::Os(format!("XTEST extension unavailable: {e}")))?;
    wait_idle(&x)?;
    let pointer = x.pointer()?;
    if let Some(id) = window {
        if !x.active().is_some_and(|a| x.transient_of(a, id as Window)) {
            x.bring_to_front(id as Window)?;
        }
    }
    let result = act(&x);
    x.conn.flush()?;
    // Let the app consume the input before the pointer moves away again.
    sleep(Duration::from_millis(100));
    // Put the pointer back only if the action moved it and no menu is open: an open
    // Qt/GTK menu grabs the pointer, and a motion outside its items clears the
    // highlighted item and closes submenus (Down then did nothing, Enter closed it).
    let moved = x.pointer().is_ok_and(|p| p != pointer);
    let menu_open = window.is_some_and(|id| !x.popups(id as Window).is_empty());
    if moved && !menu_open {
        let _ = x.motion(pointer).and_then(|_| x.sync());
    }
    result
}
