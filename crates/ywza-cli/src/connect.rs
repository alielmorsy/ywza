use std::path::{Path, PathBuf};

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

fn load_device(name: &str) -> Result<Ticket> {
    let text = std::fs::read_to_string(device_file(name))
        .with_context(|| format!("no paired device `{name}` (see `ywza devices`, `ywza pair`)"))?;
    let info: PairInfo = serde_json::from_str(&text).with_context(|| format!("device `{name}` file"))?;
    let addr = info.addr.unwrap_or_else(|| EndpointAddr::new(info.node_id));
    Ok(Ticket { addr })
}

fn load_local() -> Result<Ticket> {
    let path = config_dir().join("agent.ticket");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no {} (has ywza-agent run here?)", path.display()))?;
    Ok(text.parse()?)
}

/// Connects to the local agent or a paired device and completes the Hello/Welcome exchange.
pub async fn open(device: Option<&str>) -> Result<(Endpoint, Control)> {
    let dir = config_dir();
    let (key, ticket) = match device {
        Some(name) => ("cli.key", load_device(name)?),
        None => ("local-client.key", load_local()?),
    };
    handshake(&dir.join(key), &ticket).await
}

async fn handshake(key: &Path, ticket: &Ticket) -> Result<(Endpoint, Control)> {
    let ep = ywza_proto::bind(load_or_create_secret_key(key)?).await?;
    let mut c = connect(&ep, ticket).await?;
    let hello = ClientMsg::Hello {
        client_name: "ywza-cli".into(),
        token: None,
    };
    write_frame(&mut c.send, &hello).await?;
    match read_frame(&mut c.recv).await? {
        AgentMsg::Welcome { .. } => Ok((ep, c)),
        AgentMsg::Error { message } => bail!("agent: {message}"),
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
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), ep.close()).await;
}
