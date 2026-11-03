use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::{Ctl, Encoded, Result, Streamer};

/// Captures monitor `index` at up to `fps` (with or without the cursor) until `ctl.stop`
/// is set or `on_frame` returns false. Blocking: run it on its own thread.
pub fn run(
    index: usize,
    ctl: &Ctl,
    fps: u32,
    cursor: bool,
    mut on_frame: impl FnMut(Encoded) -> bool,
) -> Result<()> {
    let mut cap = ywza_desktop::MonitorCapture::new(index, fps, cursor)?;
    let mut s = Streamer::new(ctl.bitrate(), fps);
    let mut buf = Vec::new();
    while !ctl.stop.load(Ordering::Relaxed) {
        s.set_bitrate(ctl.bitrate());
        let force = ctl.take_keyframe();
        let frame = cap.next(Duration::from_millis(100), &mut buf)?;
        let out = match frame {
            Some((w, h)) => s.push(&buf, w, h, force)?,
            None if force => s.repeat_keyframe()?,
            None => s.flush()?,
        };
        if let Some(enc) = out {
            if !on_frame(enc) {
                break;
            }
        }
    }
    Ok(())
}
