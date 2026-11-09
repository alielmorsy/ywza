//! Live view: one H.264 stream per client, each frame on its own unidirectional QUIC
//! stream (`FrameHeader` frame + Annex-B bytes), plus human input on that monitor.

mod cursor;
mod input;
mod rate;
mod send;

use std::sync::{atomic::Ordering, Arc};

use iroh::endpoint::Connection;
use tokio::{sync::mpsc, task::JoinHandle};
use ywza_desktop::Rect;
use ywza_proto::{AgentMsg, InputEvent};
use ywza_stream::Ctl;

pub use input::to_pixels;

/// A client's running live view; dropping it stops capture and sending.
pub struct Live {
    pub monitor: u32,
    rect: Rect,
    ctl: Arc<Ctl>,
    sender: JoinHandle<()>,
}

impl Drop for Live {
    fn drop(&mut self) {
        tracing::info!(monitor = self.monitor, "live view stopped");
        self.ctl.stop.store(true, Ordering::Relaxed);
        self.sender.abort();
    }
}

impl Live {
    /// `fixed_bps` pins the bitrate and turns off adaptation; None adapts to lag.
    /// With `cursor` off, the cursor's shape goes to `to_client` as `AgentMsg::Cursor`.
    pub fn start(
        conn: Connection,
        monitor: u32,
        fps: u32,
        fixed_bps: Option<u32>,
        cursor: bool,
        to_client: mpsc::UnboundedSender<AgentMsg>,
    ) -> anyhow::Result<Self> {
        let rect = ywza_desktop::list_monitors()?
            .get(monitor as usize)
            .map(|m| m.rect)
            .ok_or_else(|| anyhow::anyhow!("no monitor {monitor}"))?;
        let ctl = Arc::new(Ctl::default());
        if let Some(bps) = fixed_bps {
            ctl.set_bitrate(bps);
        }
        let (tx, rx) = mpsc::channel(2);
        let cap_ctl = ctl.clone();
        std::thread::spawn(move || {
            let r = ywza_stream::run(monitor as usize, &cap_ctl, fps, cursor, |f| {
                tx.blocking_send(f).is_ok()
            });
            if let Err(e) = r {
                tracing::error!(monitor, error = %e, "live capture failed");
            }
        });
        if !cursor {
            let stop = ctl.clone();
            std::thread::spawn(move || cursor::watch(&stop, &to_client));
        }
        let sender = tokio::spawn(send::send_loop(
            conn,
            monitor,
            rx,
            ctl.clone(),
            fixed_bps.is_none(),
        ));
        tracing::info!(monitor, fps, adaptive = fixed_bps.is_none(), cursor, "live view started");
        Ok(Self {
            monitor,
            rect,
            ctl,
            sender,
        })
    }

    /// Human input from a client that holds `control`.
    pub fn input(&self, ev: &InputEvent) -> anyhow::Result<()> {
        input::inject(self.rect, ev)
    }
}
