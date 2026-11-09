use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use iroh::{Endpoint, EndpointAddr};
use ywza_proto::{
    bind, client_side, config_dir, load_or_create_secret_key, rendezvous_key, Error, PairInfo,
    PAIR_ALPN,
};

use crate::connect::{device_file, devices_dir, relay_url, shut};

const RETRY: &str = "wrong or expired code, run `ywza-agent pair` again";

pub async fn run(code: &str, name: &str) -> Result<()> {
    let key = load_or_create_secret_key(&config_dir().join("cli.key"))?;
    let relay = relay_url()?;
    tracing::debug!(relay = relay.as_deref().unwrap_or("default"), "pairing: binding");
    let ep = bind(key, relay.as_deref()).await?;
    let r = pair(&ep, code, name).await;
    shut(ep).await;
    r
}

async fn pair(ep: &Endpoint, code: &str, name: &str) -> Result<()> {
    let target = EndpointAddr::new(rendezvous_key(code).public());
    tracing::debug!("pairing: dialing the code's rendezvous endpoint");
    let conn = tokio::time::timeout(Duration::from_secs(30), ep.connect(target, PAIR_ALPN))
        .await
        .map_err(|_| anyhow!("no agent answered ({RETRY})"))?
        .with_context(|| format!("could not reach the agent ({RETRY})"))?;
    tracing::debug!("pairing: connected, exchanging keys");
    let (mut send, mut recv) = conn.open_bi().await?;

    let me = PairInfo {
        node_id: ep.id(),
        name: std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "ywza-cli".into()),
        addr: Some(ep.addr()),
        relay: None,
    };
    let agent = match client_side(&mut send, &mut recv, code, me).await {
        Ok(info) => info,
        Err(e @ (Error::BadCode | Error::Closed)) => {
            tracing::debug!(error = %e, "pairing: key exchange failed");
            bail!(RETRY)
        }
        Err(e) => return Err(e.into()),
    };

    tracing::debug!(agent = %agent.node_id, file = %device_file(name).display(), "pairing: saving the device");
    std::fs::create_dir_all(devices_dir())?;
    std::fs::write(device_file(name), serde_json::to_string_pretty(&agent)?)?;
    println!("paired as `{name}`; use --device {name}");
    conn.close(0u32.into(), b"bye");
    Ok(())
}
