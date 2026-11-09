use std::{sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;
use ywza_proto::AgentMsg;
use ywza_stream::Ctl;

/// How often the cursor shape is checked when it isn't drawn into the video.
const EVERY: Duration = Duration::from_millis(50);

/// Sends the cursor's shape whenever it changes, until the live view stops or the client
/// goes away.
pub fn watch(ctl: &Ctl, to_client: &mpsc::UnboundedSender<AgentMsg>) {
    let mut w = match ywza_desktop::CursorWatch::new() {
        Ok(w) => w,
        Err(e) => return tracing::warn!(error = %e, "cursor shape unavailable"),
    };
    while !ctl.stop.load(Ordering::Relaxed) {
        if let Ok(Some(s)) = w.poll() {
            let png = s.image.and_then(|i| ywza_desktop::encode_png(&i).ok());
            let msg = AgentMsg::Cursor {
                png_base64: png.map(|p| data_encoding::BASE64.encode(&p)),
                hot_x: s.hot_x,
                hot_y: s.hot_y,
            };
            if to_client.send(msg).is_err() {
                return;
            }
        }
        std::thread::sleep(EVERY);
    }
}
