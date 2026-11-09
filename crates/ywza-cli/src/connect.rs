use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{anyhow, bail, Context, Result};
use iroh::{Endpoint, EndpointAddr};
use ywza_proto::{
    config_dir, connect, load_or_create_secret_key, read_frame, write_frame, AgentMsg, ClientMsg,
    Control, PairInfo, Ticket,
};

pub fn devices_dir() -> PathBuf {
    config_dir().join("devices")
}

pub fn device_file(name: &str) -> PathBuf {
    devices_dir().join(format!("{name}.json"))
}

/// `relay_url` from the local `config.toml`; a missing file means the public relays.
pub fn relay_url() -> Result<Option<String>> {
    let path = config_dir().join("config.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(toml::from_str::<ywza_proto::Config>(&text)
            .with_context(|| format!("parsing {}", path.display()))?
            .relay_url),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// The device's ticket and the relay it was paired with, if any.
fn load_device(name: &str) -> Result<(Ticket, Option<String>)> {
    let file = device_file(name);
    tracing::debug!(device = name, file = %file.display(), "resolving the paired device");
    let text = std::fs::read_to_string(&file)
        .with_context(|| format!("no paired device `{name}` (see `ywza devices`, `ywza pair`)"))?;
    let info: PairInfo = serde_json::from_str(&text).with_context(|| format!("device `{name}` file"))?;
    let addr = info.addr.unwrap_or_else(|| EndpointAddr::new(info.node_id));
    Ok((Ticket { addr }, info.relay))
}

fn load_local() -> Result<Ticket> {
    let path = config_dir().join("agent.ticket");
    tracing::debug!(file = %path.display(), "resolving the local agent");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no {} (has ywza-agent run here?)", path.display()))?;
    Ok(text.parse()?)
}

/// Connects to the local agent or a paired device and completes the Hello/Welcome exchange.
pub async fn open(device: Option<&str>) -> Result<(Endpoint, Control)> {
    let dir = config_dir();
    let (key, ticket, relay) = match device {
        Some(name) => {
            let (ticket, relay) = load_device(name)?;
            ("cli.key", ticket, relay)
        }
        None => ("local-client.key", load_local()?, None),
    };
    // A paired device's relay wins over the local config.
    let relay = match relay {
        Some(r) => Some(r),
        None => relay_url()?,
    };
    tracing::debug!(
        device = device.unwrap_or("local"),
        key,
        relay = relay.as_deref().unwrap_or("default"),
        agent = %ticket.addr.id,
        "connecting"
    );
    handshake(&dir.join(key), &ticket, relay.as_deref()).await
}

/// One endpoint (`cli.key`) for talking to several paired devices; the first paired relay wins.
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
    Ok(ywza_proto::bind(key, relay.as_deref()).await?)
}

/// Connects to a paired device on `ep` and completes the handshake.
pub async fn dial(ep: &Endpoint, name: &str) -> Result<Control> {
    exchange(ep, &load_device(name)?.0).await
}

async fn handshake(key: &Path, ticket: &Ticket, relay: Option<&str>) -> Result<(Endpoint, Control)> {
    let key = load_or_create_secret_key(key)?;
    let ep = ywza_proto::bind(key, relay).await?;
    match exchange(&ep, ticket).await {
        Ok(c) => Ok((ep, c)),
        Err(e) => {
            shut(ep).await;
            Err(e)
        }
    }
}

async fn exchange(ep: &Endpoint, ticket: &Ticket) -> Result<Control> {
    let start = Instant::now();
    let mut c = connect(ep, ticket).await?;
    tracing::debug!(ms = start.elapsed().as_millis() as u64, "connected");
    let hello = ClientMsg::Hello {
        client_name: "ywza-cli".into(),
        token: None,
    };
    write_frame(&mut c.send, &hello).await?;
    match read_frame(&mut c.recv).await? {
        AgentMsg::Welcome {
            agent_version,
            permissions,
            ..
        } => {
            tracing::debug!(
                %agent_version,
                ?permissions,
                ms = start.elapsed().as_millis() as u64,
                "handshake done"
            );
            Ok(c)
        }
        AgentMsg::Error { message } => {
            tracing::debug!(%message, "agent refused the handshake");
            bail!("agent: {message}")
        }
        other => bail!("expected welcome, got {other:?}"),
    }
}

/// Sends one request and returns the next reply, turning `error` into Err.
pub async fn ask(c: &mut Control, msg: ClientMsg) -> Result<AgentMsg> {
    write_frame(&mut c.send, &msg).await?;
    match read_frame(&mut c.recv).await? {
        AgentMsg::Error { message } => Err(anyhow!("agent: {message}")),
        m => Ok(m),
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
