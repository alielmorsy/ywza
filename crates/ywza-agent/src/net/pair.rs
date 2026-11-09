//! Pairing with a 6-digit code. A throwaway endpoint, found by its code-derived identity,
//! accepts exactly one attempt for 10 minutes; any failure burns the code.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use iroh::{endpoint::Incoming, Endpoint, EndpointAddr};
use tokio_util::sync::CancellationToken;
use ywza_proto::{agent_side, new_code, rendezvous_key, AgentMsg, PairInfo};

use super::{bind_rendezvous, local_addr};
use crate::Agent;

pub const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);
/// Close reason sent to anyone dialing after the one attempt was used.
pub const EXPIRED: &[u8] = b"code expired, run pair again";

/// The pending code's cancel handle: a new code replaces the previous one.
#[derive(Default)]
pub struct Pairing(Mutex<Option<CancellationToken>>);

/// A pairing code and where its throwaway endpoint can be dialed directly.
pub struct Started {
    pub code: String,
    pub addr: EndpointAddr,
}

/// Opens the rendezvous endpoint for a fresh code and starts waiting for the one attempt.
pub async fn begin(agent: &Arc<Agent>) -> anyhow::Result<Started> {
    let code = new_code();
    let ep = bind_rendezvous(
        rendezvous_key(&code),
        agent.local_only,
        agent.config.get().relay_url.as_deref(),
    )
    .await?;
    if !agent.local_only {
        // Wait briefly for the relays so the code works from other networks.
        let both = async { tokio::join!(ep.online(), agent.endpoint.online()) };
        let _ = tokio::time::timeout(Duration::from_secs(5), both).await;
    }
    let addr = local_addr(&ep);
    let cancel = CancellationToken::new();
    if let Some(old) = agent.pairing.0.lock().unwrap().replace(cancel.clone()) {
        old.cancel();
    }
    tracing::info!(ttl_secs = PAIRING_TTL.as_secs(), "pairing code created");
    tokio::spawn(serve(agent.clone(), ep, code.clone(), cancel));
    Ok(Started { code, addr })
}

/// The reply to `CreatePairing`.
pub async fn create(agent: &Arc<Agent>) -> AgentMsg {
    match begin(agent).await {
        Ok(s) => AgentMsg::PairingCode {
            code: s.code,
            expires_in_secs: PAIRING_TTL.as_secs(),
        },
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "creating a pairing code failed");
            AgentMsg::Error {
                message: format!("pairing: {e:#}"),
            }
        }
    }
}

async fn serve(agent: Arc<Agent>, ep: Endpoint, code: String, cancel: CancellationToken) {
    let deadline = tokio::time::sleep(PAIRING_TTL);
    tokio::pin!(deadline);
    let mut used = false;
    loop {
        let incoming = tokio::select! {
            i = ep.accept() => i,
            _ = &mut deadline => {
                tracing::info!("pairing code expired unused");
                break;
            }
            _ = cancel.cancelled() => {
                tracing::debug!("pairing code replaced by a newer one");
                break;
            }
        };
        let Some(incoming) = incoming else { break };
        if used {
            tracing::warn!("pairing attempt on a used code refused");
            tokio::spawn(refuse(incoming));
            continue;
        }
        used = true;
        tracing::info!("pairing code used");
        match attempt(&agent, incoming, &code).await {
            Ok(()) => break,
            // The code is burnt; the endpoint stays up only to say so.
            Err(e) if matches!(e.downcast_ref::<ywza_proto::Error>(), Some(ywza_proto::Error::BadCode)) => {
                tracing::warn!("pairing code wrong, the code is burnt")
            }
            Err(e) => tracing::warn!(error = %format!("{e:#}"), "pairing failed"),
        }
    }
    ep.close().await;
}

async fn refuse(incoming: Incoming) {
    if let Ok(conn) = incoming.await {
        conn.close(4u32.into(), EXPIRED);
    }
}

async fn attempt(agent: &Agent, incoming: Incoming, code: &str) -> anyhow::Result<()> {
    let (conn, mut send, mut recv) = tokio::time::timeout(ATTEMPT_TIMEOUT, async {
        let conn = incoming.await?;
        let (send, recv) = conn.accept_bi().await?;
        anyhow::Ok((conn, send, recv))
    })
    .await??;
    tracing::debug!("pairing peer connected, running the key exchange");
    let me = PairInfo {
        node_id: agent.endpoint.id(),
        name: host_name(),
        addr: Some(local_addr(&agent.endpoint)),
        relay: agent
            .config
            .get()
            .relay_url
            .as_deref()
            .map(ywza_proto::shareable_relay),
    };
    let result = tokio::time::timeout(
        ATTEMPT_TIMEOUT,
        agent_side(&mut send, &mut recv, code, me),
    )
    .await;
    if let Ok(Ok(info)) = &result {
        trust(agent, info)?;
    }
    // The client reads our last message (or the mismatch) before it closes; do not cut it off.
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(5), conn.closed()).await;
    conn.close(2u32.into(), b"pairing finished");
    match result {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => anyhow::bail!("pairing timed out"),
    }
}

fn trust(agent: &Agent, info: &PairInfo) -> anyhow::Result<()> {
    let name = match info.name.trim() {
        "" => "device",
        n => n,
    };
    agent.config.trust(info.node_id.to_string(), name.into())?;
    tracing::info!(client = name, peer = %info.node_id, "paired a device");
    Ok(())
}

/// The machine's name, shown to the paired device.
fn host_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "ywza-agent".into())
}
