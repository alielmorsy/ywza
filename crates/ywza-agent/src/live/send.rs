use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iroh::endpoint::Connection;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use ywza_proto::{write_frame, FrameHeader};
use ywza_stream::{Ctl, Encoded};

use super::rate::RateCtl;

/// A frame in flight longer than max(this, 3x the base latency) means the link is behind.
const STALE_MIN: Duration = Duration::from_millis(400);
/// Forced keyframes are spaced out so a slow link is not flooded with them.
const KEY_GAP: Duration = Duration::from_secs(2);
const MAX_INFLIGHT: usize = 30;

struct Flight {
    token: CancellationToken,
    task: JoinHandle<()>,
    key: bool,
    sent: Instant,
}

pub async fn send_loop(
    conn: Connection,
    monitor: u32,
    mut rx: mpsc::Receiver<Encoded>,
    ctl: Arc<Ctl>,
    adaptive: bool,
) {
    let mut inflight: Vec<Flight> = Vec::new();
    let (ack_tx, mut acks) = mpsc::unbounded_channel::<(Duration, bool)>();
    let mut rate = RateCtl::new(ctl.bitrate(), Instant::now());
    let mut seq = 0u64;
    let mut need_key = false;
    let mut last_forced: Option<Instant> = None;
    while let Some(f) = rx.recv().await {
        let now = Instant::now();
        while let Ok((latency, key)) = acks.try_recv() {
            tracing::trace!(latency_ms = latency.as_millis() as u64, key, "frame acked");
            rate.on_ack(now, latency, key);
        }
        if adaptive {
            apply_rate(&ctl, &rate);
        }
        inflight.retain(|fl| !fl.task.is_finished());
        if f.keyframe {
            // Stale-frame drop: nothing older than a keyframe is worth delivering.
            for fl in inflight.drain(..) {
                fl.token.cancel();
            }
            need_key = false;
        } else if need_key || is_stale(&inflight, now, rate.base()) {
            // P-frames reference their predecessor: after a drop, wait for a keyframe.
            if !need_key {
                tracing::debug!(inflight = inflight.len(), "link behind, waiting for a keyframe");
                for fl in inflight.iter().filter(|fl| !fl.key) {
                    fl.token.cancel();
                }
                inflight.retain(|fl| fl.key);
                need_key = true;
                rate.on_stall(now);
                if adaptive {
                    apply_rate(&ctl, &rate);
                }
            }
            if last_forced.is_none_or(|t| now.duration_since(t) >= KEY_GAP) {
                ctl.request_keyframe();
                last_forced = Some(now);
            }
            continue;
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
        let sent = Instant::now();
        let Ok(mut send) = conn.open_uni().await else {
            tracing::debug!("connection closed, ending the frame sender");
            break;
        };
        tracing::trace!(seq = header.seq, bytes = f.data.len(), key = f.keyframe, "frame sent");
        let token = CancellationToken::new();
        let cancelled = token.clone();
        let ack = ack_tx.clone();
        let key = f.keyframe;
        let task = tokio::spawn(async move {
            let write = async {
                write_frame(&mut send, &header).await.ok()?;
                send.write_all(&f.data).await.ok()?;
                send.finish().ok()?;
                // In flight until the peer has acknowledged every byte.
                send.stopped().await.ok()
            };
            let outcome = tokio::select! {
                r = write => Some(r),
                _ = cancelled.cancelled() => None,
            };
            match outcome {
                None => {
                    let _ = send.reset(0u32.into());
                }
                Some(Some(_)) => {
                    let _ = ack.send((sent.elapsed(), key));
                }
                Some(None) => {}
            }
        });
        inflight.push(Flight {
            token,
            task,
            key,
            sent,
        });
    }
    for fl in inflight {
        fl.token.cancel();
    }
}

fn apply_rate(ctl: &Ctl, rate: &RateCtl) {
    let bps = rate.bitrate();
    if bps != ctl.bitrate() {
        tracing::debug!(from = ctl.bitrate(), to = bps, "bitrate changed");
        ctl.set_bitrate(bps);
    }
}

fn is_stale(inflight: &[Flight], now: Instant, base: Option<Duration>) -> bool {
    let limit = STALE_MIN.max(base.unwrap_or_default() * 3);
    inflight.len() >= MAX_INFLIGHT
        || inflight
            .iter()
            .find(|fl| !fl.key)
            .is_some_and(|fl| now.duration_since(fl.sent) > limit)
}
