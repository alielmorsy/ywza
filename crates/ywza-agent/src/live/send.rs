use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iroh::endpoint::Connection;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use ywza_proto::{write_frame, FrameHeader};
use ywza_stream::{Ctl, Encoded};

/// More unacknowledged frames than this and non-keyframes are dropped.
const MAX_BEHIND: usize = 3;
const MIN_BITRATE: u32 = 500_000;
const MAX_BITRATE: u32 = 8_000_000;
/// Raise the bitrate again after this long without lag.
const CALM: Duration = Duration::from_secs(5);

pub async fn send_loop(
    conn: Connection,
    monitor: u32,
    mut rx: mpsc::Receiver<Encoded>,
    ctl: Arc<Ctl>,
    adaptive: bool,
) {
    let mut inflight: Vec<(CancellationToken, JoinHandle<()>)> = Vec::new();
    let mut seq = 0u64;
    let mut need_key = false;
    let mut calm_since = Instant::now();
    while let Some(f) = rx.recv().await {
        inflight.retain(|(_, h)| !h.is_finished());
        if f.keyframe {
            // Stale-frame drop: nothing older than a keyframe is worth delivering.
            for (t, _) in inflight.drain(..) {
                t.cancel();
            }
            need_key = false;
        } else if need_key {
            continue;
        } else if inflight.len() > MAX_BEHIND {
            // P-frames reference their predecessor: after a drop, wait for a keyframe.
            need_key = true;
            ctl.request_keyframe();
            if adaptive {
                ctl.set_bitrate(((ctl.bitrate() as f32 * 0.6) as u32).max(MIN_BITRATE));
            }
            calm_since = Instant::now();
            continue;
        }
        if adaptive && calm_since.elapsed() > CALM && ctl.bitrate() < MAX_BITRATE {
            ctl.set_bitrate((ctl.bitrate() + ctl.bitrate() / 4).min(MAX_BITRATE));
            calm_since = Instant::now();
        }
        let header = FrameHeader {
            monitor,
            seq,
            pts_us: f.pts_us,
            keyframe: f.keyframe,
            width: f.width,
            height: f.height,
        };
        seq += 1;
        let Ok(mut send) = conn.open_uni().await else {
            break;
        };
        let token = CancellationToken::new();
        let cancelled = token.clone();
        let task = tokio::spawn(async move {
            let write = async {
                write_frame(&mut send, &header).await.ok()?;
                send.write_all(&f.data).await.ok()?;
                send.finish().ok()?;
                // In flight until the peer has acknowledged every byte.
                send.stopped().await.ok()
            };
            let reset = tokio::select! {
                _ = write => false,
                _ = cancelled.cancelled() => true,
            };
            if reset {
                let _ = send.reset(0u32.into());
            }
        });
        inflight.push((token, task));
    }
    for (t, _) in inflight {
        t.cancel();
    }
}
