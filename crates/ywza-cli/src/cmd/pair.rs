use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use iroh::EndpointAddr;
use ywza_proto::{
    bind, client_side, config_dir, load_or_create_secret_key, rendezvous_key, Error, PairInfo,
    PAIR_ALPN,
};

use crate::connect::{device_file, devices_dir, relay_url};

const RETRY: &str = "wrong or expired code, run `ywza-agent pair` again";

pub async fn run(code: &str, name: &str) -> Result<()> {
    let key = load_or_create_secret_key(&config_dir().join("cli.key"))?;
    let ep = bind(key, relay_url()?.as_deref()).await?;
    let target = EndpointAddr::new(rendezvous_key(code).public());
    let conn = tokio::time::timeout(Duration::from_secs(30), ep.connect(target, PAIR_ALPN))
        .await
        .map_err(|_| anyhow!("no agent answered ({RETRY})"))?
        .with_context(|| format!("could not reach the agent ({RETRY})"))?;
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
        Err(Error::BadCode | Error::Closed) => bail!(RETRY),
        Err(e) => return Err(e.into()),
    };

    std::fs::create_dir_all(devices_dir())?;
    std::fs::write(device_file(name), serde_json::to_string_pretty(&agent)?)?;
    println!("paired as `{name}`; use --device {name}");
    conn.close(0u32.into(), b"bye");
    let _ = tokio::time::timeout(Duration::from_secs(1), ep.close()).await;
    Ok(())
}
