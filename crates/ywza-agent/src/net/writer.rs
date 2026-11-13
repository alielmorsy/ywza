use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc};
use ywza_proto::{write_frame, AgentMsg, Permissions};

use super::peer::{redact, visible};

/// Sends `direct` replies and the broadcast `events` to one client, filtered by its current `perms`.
pub(crate) async fn write_loop<W: tokio::io::AsyncWrite + Unpin>(
    mut send: W,
    mut events: broadcast::Receiver<AgentMsg>,
    mut direct: mpsc::UnboundedReceiver<AgentMsg>,
    perms: Arc<Mutex<Permissions>>,
) {
    loop {
        let msg = tokio::select! {
            m = direct.recv() => match m { Some(m) => m, None => return },
            m = events.recv() => match m {
                Ok(m) if !visible(&m, &perms.lock().unwrap()) => continue,
                Ok(m) => m,
                Err(broadcast::error::RecvError::Lagged(n)) => AgentMsg::Error {
                    message: format!("{n} events dropped (client too slow); refetch the session"),
                },
                Err(broadcast::error::RecvError::Closed) => return,
            },
        };
        let msg = redact(msg, &perms.lock().unwrap());
        if write_frame(&mut send, &msg).await.is_err() {
            return;
        }
    }
}
