use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::sync::{broadcast, mpsc};
use ywza_proto::{write_frame, AgentMsg};

use super::peer::visible;

/// Sends `direct` replies and the broadcast `events` to one client. Session events go only
/// while `see_sessions` (chat or tools).
pub(crate) async fn write_loop<W: tokio::io::AsyncWrite + Unpin>(
    mut send: W,
    mut events: broadcast::Receiver<AgentMsg>,
    mut direct: mpsc::UnboundedReceiver<AgentMsg>,
    see_sessions: Arc<AtomicBool>,
) {
    loop {
        let msg = tokio::select! {
            m = direct.recv() => match m { Some(m) => m, None => return },
            m = events.recv() => match m {
                Ok(m) if !visible(&m, see_sessions.load(Ordering::Relaxed)) => continue,
                Ok(m) => m,
                Err(broadcast::error::RecvError::Lagged(n)) => AgentMsg::Error {
                    message: format!("{n} events dropped (client too slow); refetch the session"),
                },
                Err(broadcast::error::RecvError::Closed) => return,
            },
        };
        if write_frame(&mut send, &msg).await.is_err() {
            return;
        }
    }
}
