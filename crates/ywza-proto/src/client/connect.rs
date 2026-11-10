use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{anyhow, bail, Result};
use iroh::Endpoint;

use super::devices::{load_device, load_local, relay_url};
use crate::{
    config_dir, connect, load_or_create_secret_key, read_frame, write_frame, AgentMsg, ClientMsg,
    Control, Ticket,
};

/// The key file to dial with, the ticket and the relay for `device` (None: the local agent).
/// The local agent trusts `local-client.key`; paired devices trust `cli.key`, the key pairing used.
/// A relay recorded when pairing wins over the local config's.
pub fn resolve(device: Option<&str>) -> Result<(PathBuf, Ticket, Option<String>)> {
    let (key, ticket, relay) = match device {
        None => (config_dir().join("local-client.key"), load_local()?, None),
        Some(name) => {
            let (ticket, relay) = load_device(name)?;
            (config_dir().join("cli.key"), ticket, relay)
        }
    };
    let relay = match relay {
        Some(r) => Some(r),
        None => relay_url()?,
    };
    Ok((key, ticket, relay))
}

/// The local agent, over `local-client.key` (every permission). The Welcome message comes back too.
pub async fn open_local(client_name: &str) -> Result<(Endpoint, Control, AgentMsg)> {
    let (key, ticket, relay) = resolve(None)?;
    tracing::debug!(agent = %ticket.addr.id, "connecting to the local agent");
    handshake(&key, &ticket, relay.as_deref(), client_name).await
}

/// One paired device on its own endpoint (`cli.key`).
pub async fn open_device(name: &str, client_name: &str) -> Result<(Endpoint, Control, AgentMsg)> {
    let (key, ticket, relay) = resolve(Some(name))?;
    handshake(&key, &ticket, relay.as_deref(), client_name).await
}

/// One endpoint (`cli.key`) for talking to several paired devices; the first paired relay wins,
/// else the local config's.
pub async fn bind_devices(names: &[&str]) -> Result<Endpoint> {
    let mut relay = None;
    for name in names {
        if relay.is_none() {
            relay = load_device(name)?.1;
        }
    }
    let relay = match relay {
        Some(r) => Some(r),
        None => relay_url()?,
    };
    let key = load_or_create_secret_key(&config_dir().join("cli.key"))?;
    Ok(crate::bind(key, relay.as_deref()).await?)
}

/// Connects to a paired device on `ep` and completes the handshake. Returns the connection and
/// the device's `Welcome` message (version, OS, permissions, sessions).
pub async fn dial(ep: &Endpoint, name: &str, client_name: &str) -> Result<(Control, AgentMsg)> {
    exchange(ep, &load_device(name)?.0, client_name).await
}

async fn handshake(
    key: &Path,
    ticket: &Ticket,
    relay: Option<&str>,
    client_name: &str,
) -> Result<(Endpoint, Control, AgentMsg)> {
    let key = load_or_create_secret_key(key)?;
    let ep = crate::bind(key, relay).await?;
    match exchange(&ep, ticket, client_name).await {
        Ok((c, w)) => Ok((ep, c, w)),
        Err(e) => {
            shut(ep).await;
            Err(e)
        }
    }
}

async fn exchange(
    ep: &Endpoint,
    ticket: &Ticket,
    client_name: &str,
) -> Result<(Control, AgentMsg)> {
    let start = Instant::now();
    let mut c = connect(ep, ticket).await?;
    tracing::debug!(ms = start.elapsed().as_millis() as u64, "connected");
    let hello = ClientMsg::Hello {
        client_name: client_name.into(),
        token: None,
    };
    write_frame(&mut c.send, &hello).await?;
    match read_frame(&mut c.recv).await? {
        w @ AgentMsg::Welcome { .. } => {
            tracing::debug!(ms = start.elapsed().as_millis() as u64, "handshake done");
            Ok((c, w))
        }
        AgentMsg::Error { message } => {
            tracing::debug!(%message, "agent refused the handshake");
            bail!("agent: {message}")
        }
        other => bail!("expected welcome, got {other:?}"),
    }
}

/// Sends one request and returns the next reply, turning `error` into Err. Approval messages
/// that arrive meanwhile are skipped.
pub async fn ask(c: &mut Control, msg: ClientMsg) -> Result<AgentMsg> {
    write_frame(&mut c.send, &msg).await?;
    loop {
        match read_frame(&mut c.recv).await? {
            AgentMsg::Error { message } => return Err(anyhow!("agent: {message}")),
            AgentMsg::ApprovalRequest { .. } | AgentMsg::ApprovalResolved { .. } => {}
            m => return Ok(m),
        }
    }
}

/// Bounded: after the agent drops the link, close() can hang for many seconds, and a
/// `bridge` must exit promptly so its client sees the drop and reconnects.
pub async fn close(ep: Endpoint, c: &Control) {
    c.conn.close(0u32.into(), b"bye");
    shut(ep).await;
}

pub async fn shut(ep: Endpoint) {
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), ep.close()).await;
}
