//! File copy for one connection: `Pull` sends entries on uni streams, `Push` receives them.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use iroh::endpoint::{Connection, RecvStream};
use tokio::sync::mpsc::UnboundedSender;
use ywza_proto::{
    read_frame,
    xfer::{self, Sink},
    AgentMsg, FileHeader,
};

struct Inbound {
    sink: Arc<Sink>,
    finished: u64,
    bytes: u64,
    /// Set by `PushEnd`.
    expected: Option<u64>,
}

/// The connection's pushes in progress, by transfer id.
#[derive(Clone, Default)]
pub struct Pushes(Arc<Mutex<HashMap<u64, Inbound>>>);

fn fail(xfer: u64, path: Option<String>, message: impl ToString) -> AgentMsg {
    AgentMsg::XferError {
        xfer,
        path,
        message: message.to_string(),
    }
}

pub async fn pull(
    conn: Connection,
    xfer: u64,
    path: String,
    recursive: bool,
    direct: UnboundedSender<AgentMsg>,
) {
    let root = xfer::expand(&path);
    tracing::info!(xfer, path = %root.display(), recursive, "pull started");
    let walked = tokio::task::spawn_blocking(move || xfer::walk(&root, recursive)).await;
    let w = match walked {
        Ok(Ok(w)) => w,
        Ok(Err(e)) => return drop(direct.send(fail(xfer, None, e))),
        Err(e) => return drop(direct.send(fail(xfer, None, e))),
    };
    let _ = direct.send(AgentMsg::XferStart {
        xfer,
        name: w.name,
        files: w.entries.len() as u64,
        bytes: w.bytes,
    });
    for (rel, e) in w.errors {
        let _ = direct.send(fail(xfer, Some(rel), e));
    }
    let sent = xfer::send_all(&conn, xfer, w.entries, |rel, e| {
        let _ = direct.send(fail(xfer, Some(rel), e));
    })
    .await;
    match sent {
        Ok((files, bytes)) => {
            tracing::info!(xfer, files, bytes, "pull done");
            let _ = direct.send(AgentMsg::XferDone { xfer, files, bytes });
        }
        Err(e) => {
            tracing::warn!(xfer, error = %e, "pull failed");
            let _ = direct.send(fail(xfer, None, e));
        }
    }
}

impl Pushes {
    pub fn start(&self, xfer: u64, dest: &str, name: &str) -> AgentMsg {
        let dest = xfer::expand(dest);
        match Sink::new(&dest, name) {
            Ok(sink) => {
                tracing::info!(xfer, into = %sink.root().display(), "push started");
                let inbound = Inbound {
                    sink: Arc::new(sink),
                    finished: 0,
                    bytes: 0,
                    expected: None,
                };
                self.0.lock().unwrap().insert(xfer, inbound);
                AgentMsg::XferReady { xfer }
            }
            Err(e) => fail(xfer, None, e),
        }
    }

    pub fn end(&self, xfer: u64, files: u64) -> Option<AgentMsg> {
        let mut map = self.0.lock().unwrap();
        let Some(i) = map.get_mut(&xfer) else {
            return Some(fail(xfer, None, "unknown transfer"));
        };
        i.expected = Some(files);
        done(&mut map, xfer)
    }

    /// Serves the client's entry streams until the connection closes.
    pub async fn accept(self, conn: Connection, direct: UnboundedSender<AgentMsg>) {
        while let Ok(recv) = conn.accept_uni().await {
            let (pushes, direct) = (self.clone(), direct.clone());
            tokio::spawn(async move { pushes.receive(recv, direct).await });
        }
    }

    async fn receive(&self, mut recv: RecvStream, direct: UnboundedSender<AgentMsg>) {
        let h: FileHeader = match read_frame(&mut recv).await {
            Ok(h) => h,
            Err(e) => {
                tracing::debug!(error = %e, "unreadable entry stream");
                let _ = recv.stop(0u32.into());
                return;
            }
        };
        let sink = self.0.lock().unwrap().get(&h.xfer).map(|i| i.sink.clone());
        let Some(sink) = sink else {
            let _ = recv.stop(0u32.into());
            return;
        };
        let r = sink.write(&h, &mut recv).await;
        let mut map = self.0.lock().unwrap();
        let Some(i) = map.get_mut(&h.xfer) else {
            return;
        };
        i.finished += 1;
        match r {
            Ok(n) => i.bytes += n,
            Err(e) => {
                tracing::warn!(xfer = h.xfer, rel = h.rel, error = %e, "entry not written");
                let _ = recv.stop(0u32.into());
                // A reset stream was already reported by the sender.
                if e.kind() != std::io::ErrorKind::ConnectionReset {
                    let _ = direct.send(fail(h.xfer, Some(h.rel.clone()), e));
                }
            }
        }
        if let Some(msg) = done(&mut map, h.xfer) {
            let _ = direct.send(msg);
        }
    }
}

/// `XferDone` once every announced stream is handled; the transfer is then forgotten.
fn done(map: &mut HashMap<u64, Inbound>, xfer: u64) -> Option<AgentMsg> {
    let i = map.get(&xfer)?;
    if i.expected != Some(i.finished) {
        return None;
    }
    let (files, bytes) = (i.finished, i.bytes);
    map.remove(&xfer);
    tracing::info!(xfer, files, bytes, "push done");
    Some(AgentMsg::XferDone { xfer, files, bytes })
}
