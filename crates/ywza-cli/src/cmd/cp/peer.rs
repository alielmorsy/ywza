use anyhow::{anyhow, bail, Result};
use iroh::endpoint::{Connection, SendStream};
use tokio::{sync::mpsc, task::JoinHandle};
use ywza_proto::{read_frame, write_frame, AgentMsg, ClientMsg, Control};

use super::Stats;

/// One device connection used by a copy. Replies are read in their own task, so waiting for
/// one can sit in a `select!` beside the entry streams without losing half-read frames.
pub struct Peer {
    pub name: String,
    pub conn: Connection,
    send: SendStream,
    msgs: mpsc::UnboundedReceiver<AgentMsg>,
    reader: JoinHandle<()>,
}

impl Peer {
    pub fn new(name: &str, c: Control) -> Self {
        let (tx, msgs) = mpsc::unbounded_channel();
        let mut recv = c.recv;
        let reader = tokio::spawn(async move {
            while let Ok(m) = read_frame::<_, AgentMsg>(&mut recv).await {
                if tx.send(m).is_err() {
                    return;
                }
            }
        });
        Self {
            name: name.into(),
            conn: c.conn,
            send: c.send,
            msgs,
            reader,
        }
    }

    pub async fn tell(&mut self, msg: &ClientMsg) -> Result<()> {
        Ok(write_frame(&mut self.send, msg).await?)
    }

    pub async fn recv(&mut self) -> Result<AgentMsg> {
        self.msgs
            .recv()
            .await
            .ok_or_else(|| anyhow!("{}: connection lost", self.name))
    }

    /// `m` if it is a reply for `xfer`. Entry errors are reported and skipped; a failed
    /// transfer is Err.
    pub fn check(&self, xfer: u64, m: AgentMsg, st: &mut Stats) -> Result<Option<AgentMsg>> {
        match m {
            AgentMsg::XferError {
                xfer: x,
                path: Some(path),
                message,
            } if x == xfer => st.error(format!("{}:{path}: {message}", self.name)),
            AgentMsg::XferError {
                xfer: x, message, ..
            } if x == xfer => bail!("{}: {message}", self.name),
            AgentMsg::Error { message } => bail!("{}: {message}", self.name),
            AgentMsg::XferStart { xfer: x, .. }
            | AgentMsg::XferReady { xfer: x }
            | AgentMsg::XferDone { xfer: x, .. }
                if x == xfer =>
            {
                return Ok(Some(m))
            }
            _ => {}
        }
        Ok(None)
    }

    pub async fn next(&mut self, xfer: u64, st: &mut Stats) -> Result<AgentMsg> {
        loop {
            let m = self.recv().await?;
            if let Some(m) = self.check(xfer, m, st)? {
                return Ok(m);
            }
        }
    }

    pub fn close(self) {
        self.reader.abort();
        self.conn.close(0u32.into(), b"bye");
    }
}
