use crate::{platform, Result, Snapshot, WindowId};
use std::time::{Duration, Instant};

/// Default `timeout` for [`wait_settled`].
pub const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

/// No UI event for this long counts as settled when events are trustworthy.
const QUIET: Duration = Duration::from_millis(150);

/// What a window's event mirror saw. `live` false means events are unavailable or
/// untrusted for this window and the caller must poll; `count` only grows.
#[derive(Debug, Clone, Copy)]
pub struct Activity {
    pub count: u64,
    pub changed: bool,
    pub live: bool,
}

/// Waits until `window` stops changing, or `timeout` passes (then the latest snapshot is
/// returned, not an error). With working UI events it waits for ~150 ms without any and
/// snapshots once; otherwise it re-snapshots every ~150 ms until two consecutive
/// snapshots render identically. IDs are stable across the snapshots, so the result can
/// be used for actions.
pub fn wait_settled(window: WindowId, timeout: Duration) -> Result<Snapshot> {
    let deadline = Instant::now() + timeout;
    let start = platform::wait_activity(window, u64::MAX, Duration::ZERO);
    let mut last = platform::snapshot(window, None)?;
    if start.live {
        let (mut count, mut seen) = (start.count, false);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let a = platform::wait_activity(window, count, QUIET.min(left));
            if !a.changed {
                break;
            }
            (count, seen) = (a.count, true);
            if Instant::now() >= deadline {
                break;
            }
        }
        let next = platform::snapshot(window, Some(&last))?;
        if seen || next.same_content(&last) {
            return Ok(next);
        }
        // Changed without a single event: the provider drops them, poll instead.
        platform::mark_silent(window);
        last = next;
    }
    loop {
        std::thread::sleep(Duration::from_millis(150));
        let next = platform::snapshot(window, Some(&last))?;
        if next.same_content(&last) || Instant::now() >= deadline {
            return Ok(next);
        }
        last = next;
    }
}
