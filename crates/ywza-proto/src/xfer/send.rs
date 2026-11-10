use iroh::endpoint::Connection;
use tokio::{
    io::{AsyncWrite, BufReader},
    task::JoinSet,
};

use super::Entry;
use crate::{write_frame, Error};

/// Entry streams in flight at once; small files would crawl one after another.
const PARALLEL: usize = 8;

/// Writes one entry: its header, then the file's bytes. Returns the bytes sent.
pub async fn send_entry<W: AsyncWrite + Unpin>(
    w: &mut W,
    e: &Entry,
    xfer: u64,
) -> Result<u64, Error> {
    let mut h = e.header.clone();
    h.xfer = xfer;
    write_frame(w, &h).await?;
    if h.dir {
        return Ok(0);
    }
    let f = tokio::fs::File::open(&e.path).await?;
    let mut r = BufReader::with_capacity(256 * 1024, f);
    Ok(tokio::io::copy_buf(&mut r, w).await?)
}

/// Sends every entry on its own uni stream. A failed entry goes to `on_err` (relative path,
/// error) and the rest continue; only a dead connection fails the whole call.
/// Returns (streams opened, bytes sent).
pub async fn send_all(
    conn: &Connection,
    xfer: u64,
    entries: Vec<Entry>,
    mut on_err: impl FnMut(String, Error),
) -> Result<(u64, u64), Error> {
    let mut set = JoinSet::new();
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut reap = |r: Result<(String, Result<u64, Error>), tokio::task::JoinError>| match r {
        Ok((_, Ok(n))) => bytes += n,
        Ok((rel, Err(e))) => on_err(rel, e),
        Err(e) => on_err(String::new(), Error::transport(e)),
    };
    for e in entries {
        while set.len() >= PARALLEL {
            if let Some(r) = set.join_next().await {
                reap(r);
            }
        }
        let mut send = conn.open_uni().await.map_err(Error::transport)?;
        files += 1;
        set.spawn(async move {
            let r = async {
                let n = send_entry(&mut send, &e, xfer).await?;
                send.finish().map_err(Error::transport)?;
                Ok(n)
            }
            .await;
            if r.is_err() {
                let _ = send.reset(0u32.into());
            }
            (e.header.rel, r)
        });
    }
    while let Some(r) = set.join_next().await {
        reap(r);
    }
    Ok((files, bytes))
}
