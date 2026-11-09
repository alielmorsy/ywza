//! The three copy directions. Each runs one transfer (`xfer`) for one source.

use std::{io::ErrorKind, path::Path, sync::Arc};

use anyhow::{anyhow, bail, Result};
use iroh::endpoint::RecvStream;
use tokio::task::JoinSet;
use ywza_proto::{
    read_frame, write_frame,
    xfer::{self, Sink},
    AgentMsg, ClientMsg, FileHeader,
};

use super::{peer::Peer, Stats};

/// A reset stream was already reported by its sender.
fn reported_elsewhere(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<std::io::Error>()
            .is_some_and(|e| e.kind() == ErrorKind::ConnectionReset)
    })
}

/// Starts a pull on `p`; returns the source's file name.
async fn start_pull(p: &mut Peer, xfer: u64, path: &str, recursive: bool, st: &mut Stats) -> Result<String> {
    let path = path.to_string();
    p.tell(&ClientMsg::Pull {
        xfer,
        path,
        recursive,
    })
    .await?;
    match p.next(xfer, st).await? {
        AgentMsg::XferStart { name, .. } => Ok(name),
        m => bail!("{}: unexpected reply {m:?}", p.name),
    }
}

async fn start_push(p: &mut Peer, xfer: u64, dest: &str, name: &str, st: &mut Stats) -> Result<()> {
    let (dest, name) = (dest.to_string(), name.to_string());
    p.tell(&ClientMsg::Push { xfer, dest, name }).await?;
    match p.next(xfer, st).await? {
        AgentMsg::XferReady { .. } => Ok(()),
        m => bail!("{}: unexpected reply {m:?}", p.name),
    }
}

async fn end_push(p: &mut Peer, xfer: u64, files: u64, st: &mut Stats) -> Result<()> {
    p.tell(&ClientMsg::PushEnd { xfer, files }).await?;
    loop {
        if let AgentMsg::XferDone { files, bytes, .. } = p.next(xfer, st).await? {
            st.files += files;
            st.bytes += bytes;
            return Ok(());
        }
    }
}

/// Runs `handle` on every entry stream the agent sends for a pull, until `XferDone` and all
/// handled. Returns (streams arrived, bytes the handlers reported).
async fn each_stream<F, Fut>(p: &mut Peer, xfer: u64, st: &mut Stats, handle: F) -> Result<(u64, u64)>
where
    F: Fn(RecvStream) -> Fut,
    Fut: std::future::Future<Output = Result<u64>> + Send + 'static,
{
    enum Ev {
        Msg(Result<AgentMsg>),
        Stream(Result<RecvStream, iroh::endpoint::ConnectionError>),
        Done(Result<Result<u64>, tokio::task::JoinError>),
    }
    let conn = p.conn.clone();
    let mut set = JoinSet::new();
    let (mut arrived, mut finished, mut bytes, mut expected) = (0u64, 0u64, 0u64, None);
    while expected != Some(finished) {
        let ev = tokio::select! {
            m = p.recv() => Ev::Msg(m),
            r = conn.accept_uni() => Ev::Stream(r),
            Some(r) = set.join_next() => Ev::Done(r),
        };
        match ev {
            Ev::Msg(m) => {
                if let Some(AgentMsg::XferDone { files, .. }) = p.check(xfer, m?, st)? {
                    expected = Some(files);
                }
            }
            Ev::Stream(r) => {
                let recv = r.map_err(|e| anyhow!("{}: {e}", p.name))?;
                arrived += 1;
                set.spawn(handle(recv));
            }
            Ev::Done(r) => {
                finished += 1;
                match r? {
                    Ok(n) => bytes += n,
                    Err(e) if reported_elsewhere(&e) => {}
                    Err(e) => st.error(format!("{e:#}")),
                }
            }
        }
    }
    Ok((arrived, bytes))
}

/// device:path to a local destination.
pub async fn pull(p: &mut Peer, xfer: u64, path: &str, recursive: bool, dst: &Path, st: &mut Stats) -> Result<()> {
    let name = start_pull(p, xfer, path, recursive, st).await?;
    let sink = Arc::new(Sink::new(dst, &name)?);
    let shown = sink.root().display().to_string();
    let (files, bytes) = each_stream(p, xfer, st, move |mut recv| {
        let (sink, shown) = (sink.clone(), shown.clone());
        async move {
            let h: FileHeader = read_frame(&mut recv).await?;
            sink.write(&h, &mut recv)
                .await
                .map_err(|e| anyhow::Error::new(e).context(format!("{shown}/{}", h.rel)))
        }
    })
    .await?;
    st.files += files;
    st.bytes += bytes;
    Ok(())
}

/// A local source to device:dest.
pub async fn push(p: &mut Peer, xfer: u64, src: &Path, recursive: bool, dest: &str, st: &mut Stats) -> Result<()> {
    let src = src.to_path_buf();
    let w = tokio::task::spawn_blocking(move || xfer::walk(&src, recursive)).await??;
    for (rel, e) in &w.errors {
        st.error(format!("{rel}: {e}"));
    }
    start_push(p, xfer, dest, &w.name, st).await?;
    let conn = p.conn.clone();
    let (files, _) = xfer::send_all(&conn, xfer, w.entries, |rel, e| st.error(format!("{rel}: {e}"))).await?;
    end_push(p, xfer, files, st).await
}

/// device:path to other-device:dest, piped through this machine without touching its disk.
pub async fn relay(
    a: &mut Peer,
    b: &mut Peer,
    xfer: u64,
    path: &str,
    recursive: bool,
    dest: &str,
    st: &mut Stats,
) -> Result<()> {
    let name = start_pull(a, xfer, path, recursive, st).await?;
    start_push(b, xfer, dest, &name, st).await?;
    let to = b.conn.clone();
    let (files, _) = each_stream(a, xfer, st, move |mut recv| {
        let to = to.clone();
        async move {
            let mut send = to.open_uni().await?;
            let r = async {
                let h: FileHeader = read_frame(&mut recv).await?;
                write_frame(&mut send, &h).await?;
                let n = tokio::io::copy(&mut recv, &mut send).await?;
                send.finish()?;
                anyhow::Ok(n)
            }
            .await;
            if r.is_err() {
                let _ = send.reset(0u32.into());
            }
            r
        }
    })
    .await?;
    end_push(b, xfer, files, st).await
}
