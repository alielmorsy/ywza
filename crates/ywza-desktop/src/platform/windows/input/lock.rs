use crate::platform::windows::util::hwnd;
use crate::{Error, Result, WindowId};
use ::windows::core::HSTRING;
use ::windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use ::windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use std::time::Duration;

/// How long an action waits for another process to finish with a window (or the foreground).
pub(super) const LOCK_WAIT: Duration = Duration::from_secs(15);

/// A named, cross-process mutex held until dropped. Must be dropped on the thread that took it.
struct Lock(HANDLE);

impl Lock {
    fn acquire(name: &str, timeout: Duration) -> Result<Lock> {
        let h = unsafe { CreateMutexW(None, false, &HSTRING::from(name))? };
        let lock = Lock(h); // closes the handle on every early return
        match unsafe {
            WaitForSingleObject(h, timeout.as_millis().min(u32::MAX as u128 - 1) as u32)
        } {
            // An abandoned mutex (owner died mid-action) is ours now.
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(lock),
            _ => Err(Error::Busy), // drop's ReleaseMutex fails harmlessly: not owned
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

/// Exclusive right to act on one window, across processes, for as long as it lives.
/// Actions that use real input (SendInput / the cursor) also hold the global lock,
/// because that input goes to whatever is in front. Order is always global, then window.
/// The live-view input in `stream_capture` is deliberately not locked: the human's
/// control must never block behind an agent action.
pub(super) struct InputLock {
    _window: Option<Lock>, // declared first: released before the global lock
    _global: Option<Lock>,
}

impl InputLock {
    /// Background paths (UIA patterns, posted messages): the window lock only.
    pub(super) fn window(window: WindowId) -> Result<InputLock> {
        Self::take(false, Some(window), LOCK_WAIT)
    }

    /// Real input: global lock, then the window's (if any).
    pub(super) fn foreground(window: Option<WindowId>) -> Result<InputLock> {
        Self::take(true, window, LOCK_WAIT)
    }

    fn take(global: bool, window: Option<WindowId>, timeout: Duration) -> Result<InputLock> {
        let _global = global
            .then(|| Lock::acquire(r"Local\ywza-input-global", timeout))
            .transpose()?;
        let _window = window
            .map(|w| Lock::acquire(&window_lock_name(w), timeout))
            .transpose()?;
        Ok(InputLock { _window, _global })
    }
}

fn window_lock_name(window: WindowId) -> String {
    format!(r"Local\ywza-input-{:x}", hwnd(window).0 as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Instant;

    /// Window ids unique to this process so parallel test runs never collide.
    fn id(n: u64) -> WindowId {
        (std::process::id() as u64) << 8 | n
    }

    #[test]
    fn second_acquirer_waits_for_first() {
        let w = id(1);
        let (ready_tx, ready) = channel();
        let first = thread::spawn(move || {
            let _l = InputLock::take(false, Some(w), LOCK_WAIT).unwrap();
            ready_tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(400));
        });
        ready.recv().unwrap();
        let start = Instant::now();
        let _l = InputLock::take(false, Some(w), Duration::from_secs(5)).unwrap();
        assert!(
            start.elapsed() >= Duration::from_millis(250),
            "got the lock while it was held"
        );
        first.join().unwrap();
    }

    #[test]
    fn times_out_busy_then_succeeds_after_release() {
        let w = id(2);
        let (ready_tx, ready) = channel();
        let (release_tx, release) = channel::<()>();
        let holder = thread::spawn(move || {
            let _l = InputLock::take(false, Some(w), LOCK_WAIT).unwrap();
            ready_tx.send(()).unwrap();
            release.recv().unwrap();
        });
        ready.recv().unwrap();
        assert!(matches!(
            InputLock::take(false, Some(w), Duration::from_millis(100)),
            Err(Error::Busy)
        ));
        // other windows and a different name are unaffected
        InputLock::take(false, Some(id(3)), Duration::from_millis(100)).unwrap();
        release_tx.send(()).unwrap();
        holder.join().unwrap();
        InputLock::take(false, Some(w), Duration::from_millis(100)).unwrap();
    }
}
