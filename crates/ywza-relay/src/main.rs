//! ywza-relay: a small iroh relay server. Devices use it via `relay_url` in their config.

mod config;
mod log;
mod selfsigned;
mod tls;

use std::{net::Ipv6Addr, path::PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use iroh_relay::server::{QuicConfig, RelayConfig, Server, ServerConfig, TlsConfig};

use config::Config;

const DEV_HTTP_PORT: u16 = 3340;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Path to the TOML config file.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Plain HTTP on [::]:3340, no TLS, no QUIC. Ignores the config file.
    #[arg(long)]
    dev: bool,
}

/// Logging settings for early init; a bad config is reported by `build`.
fn log_config(args: &Args) -> log::LogConfig {
    args.config
        .as_ref()
        .filter(|_| !args.dev)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| toml::from_str::<Config>(&t).ok())
        .map(|c| c.log)
        .unwrap_or_default()
}

fn build(args: &Args) -> Result<ServerConfig> {
    let mut server = ServerConfig::default();
    if args.dev {
        server.relay = Some(RelayConfig::new((Ipv6Addr::UNSPECIFIED, DEV_HTTP_PORT)));
        return Ok(server);
    }
    let Some(path) = &args.config else {
        bail!("pass --config <path> (see relay.example.toml) or --dev");
    };
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: Config =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let mut relay = RelayConfig::new((Ipv6Addr::UNSPECIFIED, cfg.http_port));
    if !cfg.tls {
        server.relay = Some(relay);
        return Ok(server);
    }
    relay.tls = Some(TlsConfig::new(
        (Ipv6Addr::UNSPECIFIED, cfg.https_port),
        tls::cert_config(&cfg)?,
    ));
    server.relay = Some(relay);
    server.quic = cfg
        .quic_addr_discovery
        .then(|| QuicConfig::new((Ipv6Addr::UNSPECIFIED, cfg.quic_port)));
    Ok(server)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _log_guard = log::init(&log_config(&args))?;
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to set default crypto provider");

    let config = build(&args)?;
    let mut server = Server::spawn(config)
        .await
        .map_err(|e| anyhow!("starting relay: {e:#}"))?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "ywza-relay starting");
    if let Some(addr) = server.http_addr() {
        tracing::info!("listening on http://{addr}");
    }
    match server.https_addr() {
        Some(addr) => tracing::info!("listening on https://{addr} (tls on)"),
        None => tracing::info!("tls off"),
    }
    if let Some(addr) = server.quic_addr() {
        tracing::info!("listening on quic {addr}");
    }
    tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => tracing::info!("shutting down: Ctrl-C"),
        res = server.join() => tracing::error!("shutting down: server exited: {res:?}"),
    }
    server
        .shutdown()
        .await
        .map_err(|e| anyhow!("shutting down: {e:#}"))
}
