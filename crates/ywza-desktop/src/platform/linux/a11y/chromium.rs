//! Chromium and Electron export their AT-SPI tree only after the desktop says a screen
//! reader is on (`org.a11y.Status.ScreenReaderEnabled`); `connect` sets only `IsEnabled`.

use super::bus::{block, stage, step};
use super::node::Handle;
use crate::snapshot::Raw;
use crate::WindowInfo;
use atspi::proxy::bus::StatusProxy;
use atspi::zbus;
use std::sync::Mutex;
use std::time::Duration;

const NAMES: &[&str] = &[
    "chrome", "chromium", "electron", "code", "slack", "discord", "brave", "msedge",
];

static NUDGED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn lacks_document(raws: &[(Raw, Option<Handle>)]) -> bool {
    raws.iter().all(|(r, _)| r.role != "document")
}

/// By executable name, else by the `icudtl.dat` every Chromium and Electron build
/// ships next to its binary.
fn is_chromium(process: &str, pid: u32) -> bool {
    NAMES.contains(&process.to_ascii_lowercase().as_str())
        || std::fs::read_link(format!("/proc/{pid}/exe"))
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("icudtl.dat")))
            .is_some_and(|f| f.exists())
}

/// A walk with no web document in a Chromium-based app, not nudged before.
pub(super) fn needs_wake(raws: &[(Raw, Option<Handle>)], win: &WindowInfo) -> bool {
    lacks_document(raws)
        && is_chromium(&win.process, win.pid)
        && !NUDGED.lock().is_ok_and(|n| n.contains(&win.pid))
}

/// Turns ScreenReaderEnabled on and gives the app a moment to export its tree. True
/// when it was switched on, so the caller walks again. If something already had it on,
/// there is nothing more to ask.
pub(super) fn wake(pid: u32) -> bool {
    if let Ok(mut n) = NUDGED.lock() {
        n.push(pid);
    }
    block(async {
        let session = stage("session bus", zbus::Connection::session()).await?;
        let status = stage("a11y status", StatusProxy::new(&session)).await?;
        if step(status.screen_reader_enabled()).await != Some(false) {
            return Ok(false);
        }
        step(status.set_screen_reader_enabled(true)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(true)
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection() {
        assert!(lacks_document(&[]));
        assert!(is_chromium("Chrome", 0));
        assert!(!is_chromium("xcode", 0));
    }
}
