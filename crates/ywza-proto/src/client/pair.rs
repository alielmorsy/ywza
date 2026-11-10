use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use iroh::EndpointAddr;

use super::{
    connect::shut,
    devices::{device_file, devices_dir, relay_url, valid_device_name},
};
use crate::{
    bind, client_side, config_dir, load_or_create_secret_key, rendezvous_key, Error, PairInfo,
    PAIR_ALPN,
};

const RETRY: &str = "wrong or expired code, run `ywza-agent pair` again";

/// Pairs this machine's `cli.key` identity with the device showing `code` and saves it as the
/// device file `name`.
pub async fn pair(code: &str, name: &str) -> Result<()> {
    if !valid_device_name(name) {
        bail!(
            "bad device name `{name}`: use letters, digits, - and _ (\"local\" means this machine)"
        );
    }
    let key = load_or_create_secret_key(&config_dir().join("cli.key"))?;
    let relay = relay_url()?;
    tracing::debug!(
        relay = relay.as_deref().unwrap_or("default"),
        "pairing: binding"
    );
    let ep = bind(key, relay.as_deref()).await?;
    let r = exchange(&ep, code, name).await;
    shut(ep).await;
    r
}

async fn exchange(ep: &iroh::Endpoint, code: &str, name: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let target = EndpointAddr::new(rendezvous_key(code).public());
        tracing::debug!("pairing: dialing the code's rendezvous endpoint");
        let conn = ep
            .connect(target, PAIR_ALPN)
            .await
            .with_context(|| format!("could not reach the agent ({RETRY})"))?;
        let r = talk(ep, &conn, code, name).await;
        conn.close(0u32.into(), b"bye");
        r
    })
    .await
    .map_err(|_| anyhow!("no agent answered ({RETRY})"))?
}

async fn talk(
    ep: &iroh::Endpoint,
    conn: &iroh::endpoint::Connection,
    code: &str,
    name: &str,
) -> Result<()> {
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

    // The local agent already is "This PC"; a device entry for it only causes confusion.
    if super::connect::resolve(None).is_ok_and(|(_, local, _)| local.addr.id == agent.node_id) {
        bail!(
            "that code is from this PC's own agent; pair with the code shown on the other device"
        );
    }
    tracing::debug!(agent = %agent.node_id, file = %device_file(name).display(), "pairing: saving the device");
    std::fs::create_dir_all(devices_dir())?;
    std::fs::write(device_file(name), serde_json::to_string_pretty(&agent)?)?;
    Ok(())
}
