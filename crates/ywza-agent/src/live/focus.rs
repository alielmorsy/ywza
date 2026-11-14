use std::{sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;
use ywza_desktop::Rect;
use ywza_proto::AgentMsg;
use ywza_stream::Ctl;

use crate::inspect::rel_rect;

/// Focus checks go through UIA, which can take a while in a busy app; a few per second
/// keeps the caret followable without loading the desktop.
const EVERY: Duration = Duration::from_millis(250);

/// Sends where text would go (relative to `monitor`) whenever it changes, until the live
/// view stops or the client goes away.
pub fn watch(ctl: &Ctl, monitor: Rect, to_client: &mpsc::UnboundedSender<AgentMsg>) {
    let w = match ywza_desktop::FocusWatch::new() {
        Ok(w) => w,
        Err(e) => return tracing::warn!(error = %e, "focus detection unavailable"),
    };
    let mut last = None;
    while !ctl.stop.load(Ordering::Relaxed) {
        let rect = w.now().ok().flatten().map(|r| rel_rect((monitor.x, monitor.y), r));
        if last != Some(rect) {
            last = Some(rect);
            if to_client.send(AgentMsg::Focus { rect }).is_err() {
                return;
            }
        }
        std::thread::sleep(EVERY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_is_relative_to_the_monitor() {
        let r = rel_rect((-1920, 100), Rect::new(-1900, 150, 2, 18));
        assert_eq!((r.x, r.y, r.width, r.height), (20, 50, 2, 18));
    }
}
