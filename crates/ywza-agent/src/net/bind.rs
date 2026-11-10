use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::Path,
};

use iroh::{
    endpoint::{presets, BindOpts},
    Endpoint, EndpointAddr, RelayMode, SecretKey, TransportAddr,
};
use ywza_proto::{ALPN, PAIR_ALPN};

/// The UDP port saved by an earlier run; 0 (any port) when there is none.
pub fn saved_port(path: &Path) -> u16 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Binds the agent's endpoint. Remote-reachable agents reuse the UDP port saved in
/// `<dir>/agent.port`, so the direct addresses in paired tickets survive restarts: clients
/// reconnect on a LAN even when no relay or address lookup is reachable.
pub async fn bind(
    key: SecretKey,
    local_only: bool,
    dir: &Path,
    relay: Option<&str>,
) -> anyhow::Result<Endpoint> {
    if local_only {
        return loopback(key, ALPN).await;
    }
    let path = dir.join("agent.port");
    let saved = saved_port(&path);
    tracing::debug!(
        saved_port = saved,
        relay = relay.unwrap_or("default"),
        "binding the endpoint"
    );
    let ep = match bind_port(key.clone(), saved, relay).await {
        Ok(ep) => ep,
        Err(e) if saved != 0 => {
            tracing::warn!(port = saved, error = %format!("{e:#}"), "saved UDP port unavailable, taking a new one");
            bind_port(key, 0, relay).await?
        }
        Err(e) => return Err(e),
    };
    if let Some(a) = ep.bound_sockets().iter().find(|a| a.is_ipv4()) {
        let _ = std::fs::write(&path, a.port().to_string());
    }
    Ok(ep)
}

/// The n0 defaults (public relays unless `relay` is set, address lookup) with the IP sockets on
/// `port` (0 = random).
async fn bind_port(key: SecretKey, port: u16, relay: Option<&str>) -> anyhow::Result<Endpoint> {
    let b = ywza_proto::with_relay(Endpoint::builder(presets::N0), relay)?;
    Ok(b.secret_key(key)
        .alpns(vec![ALPN.to_vec()])
        .dns_resolver(ywza_proto::dns_resolver())
        .clear_ip_transports()
        .bind_addr(format!("0.0.0.0:{port}").as_str())?
        .bind_addr_with_opts(
            format!("[::]:{port}").as_str(),
            BindOpts::default().set_is_required(false),
        )?
        .bind()
        .await?)
}

/// The short-lived endpoint a pairing code is found at: n0 discovery, any port.
pub async fn bind_rendezvous(
    key: SecretKey,
    local_only: bool,
    relay: Option<&str>,
) -> anyhow::Result<Endpoint> {
    if local_only {
        return loopback(key, PAIR_ALPN).await;
    }
    let b = ywza_proto::with_relay(Endpoint::builder(presets::N0), relay)?;
    Ok(b.secret_key(key)
        .alpns(vec![PAIR_ALPN.to_vec()])
        .dns_resolver(ywza_proto::dns_resolver())
        .bind()
        .await?)
}

async fn loopback(key: SecretKey, alpn: &[u8]) -> anyhow::Result<Endpoint> {
    Ok(Endpoint::builder(presets::Minimal)
        .secret_key(key)
        .relay_mode(RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")?
        .alpns(vec![alpn.to_vec()])
        .bind()
        .await?)
}

/// Current address plus loopback versions of the bound sockets, so local clients always connect.
pub fn local_addr(ep: &Endpoint) -> EndpointAddr {
    let loopback = ep.bound_sockets().into_iter().map(|mut s| {
        if s.ip().is_unspecified() {
            s.set_ip(match s.ip() {
                IpAddr::V4(_) => Ipv4Addr::LOCALHOST.into(),
                IpAddr::V6(_) => Ipv6Addr::LOCALHOST.into(),
            });
        }
        TransportAddr::Ip(s)
    });
    ep.addr().with_addrs(loopback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_port_reads_the_file_or_falls_back_to_any() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.port");
        assert_eq!(saved_port(&path), 0);
        std::fs::write(&path, "43210\n").unwrap();
        assert_eq!(saved_port(&path), 43210);
        std::fs::write(&path, "not a port").unwrap();
        assert_eq!(saved_port(&path), 0);
    }

    #[tokio::test]
    async fn loopback_endpoints_bind_and_report_a_loopback_address() {
        let ep = loopback(SecretKey::generate(), ALPN).await.unwrap();
        let addr = local_addr(&ep);
        assert_eq!(addr.id, ep.id());
        assert!(addr.ip_addrs().all(|a| a.ip().is_loopback()));
        ep.close().await;
    }
}
