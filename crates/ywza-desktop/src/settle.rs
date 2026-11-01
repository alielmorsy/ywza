use crate::{platform, Result, Snapshot, WindowId};
use std::time::{Duration, Instant};

/// Default `timeout` for [`wait_settled`].
pub const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

/// Re-snapshots `window` every ~150 ms until two consecutive snapshots render
/// identically, or `timeout` passes (then the latest one is returned, not an error).
/// IDs are stable across the snapshots, so the result can be used for actions.
pub fn wait_settled(window: WindowId, timeout: Duration) -> Result<Snapshot> {
    let deadline = Instant::now() + timeout;
    let mut last = platform::snapshot(window, None)?;
    loop {
        std::thread::sleep(Duration::from_millis(150));
        let next = platform::snapshot(window, Some(&last))?;
        if next.same_content(&last) || Instant::now() >= deadline {
            return Ok(next);
        }
        last = next;
    }
}
