//! `ywza-agent [run|pair|ticket|service]`
//!
//! - `run` (default): the service.
//! - `pair`: asks the running agent for a one-time 6-digit pairing code (via
//!   `<config_dir>/pairing-request`, answered in `<config_dir>/pairing.code`) and prints it.
//!   Valid 10 minutes, single use.
//! - `ticket`: prints `<config_dir>/agent.ticket` (address only; for local clients holding `local-client.key`).
//! - `service [--user <name>]` (Linux, root): keeps a `run` agent on whatever X11 session seat0
//!   shows; see `platform/linux/service`.

use std::{path::PathBuf, time::Duration};

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    // Console attach runs in a child copy of this binary; that copy exits here.
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = ywza_term::helper_main(&args) {
        std::process::exit(code);
    }
    if args.get(1).map(String::as_str) == Some("service") {
        if let Some(result) = ywza_agent::platform::service(&args) {
            return result;
        }
    }
    // Before any window/capture call, so every rect is in physical pixels.
    if let Err(e) = ywza_desktop::set_dpi_awareness() {
        eprintln!("ywza-agent: DPI awareness: {e}");
    }
    let dir = ywza_proto::config_dir();
    let cmd = args.get(1).map_or("run", String::as_str);
    let rt = tokio::runtime::Runtime::new()?;
    match cmd {
        "run" => rt.block_on(async {
            let agent = ywza_agent::Agent::start(dir, false).await?;
            eprintln!(
                "ywza-agent {} listening as {}",
                env!("CARGO_PKG_VERSION"),
                agent.endpoint.id()
            );
            tokio::signal::ctrl_c().await?;
            agent.endpoint.close().await;
            Ok(())
        }),
        "pair" => rt.block_on(pair(dir)),
        "ticket" => {
            let path = dir.join("agent.ticket");
            let t = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {} (has the agent run yet?)", path.display()))?;
            println!("{}", t.trim());
            Ok(())
        }
        _ => {
            eprintln!("usage: ywza-agent [run|pair|ticket|service [--user <name>]]");
            std::process::exit(2);
        }
    }
}

async fn pair(dir: PathBuf) -> anyhow::Result<()> {
    let answer = dir.join("pairing.code");
    let _ = std::fs::remove_file(&answer);
    std::fs::write(dir.join("pairing-request"), b"")
        .with_context(|| format!("writing to {}", dir.display()))?;
    // The agent may wait a few seconds for the relays before it answers.
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let Ok(text) = std::fs::read_to_string(&answer) else {
            continue;
        };
        let _ = std::fs::remove_file(&answer);
        let text = text.trim();
        if let Some(e) = text.strip_prefix("error:") {
            anyhow::bail!("{}", e.trim());
        }
        let (code, secs) = text.split_once(' ').unwrap_or((text, "600"));
        println!("{code}");
        let minutes = secs.parse::<u64>().map_or(10, |s| s.div_ceil(60));
        eprintln!(
            "Valid for {minutes} minutes, one device, one try. On the other device run: ywza pair {code} --name <name>"
        );
        return Ok(());
    }
    let _ = std::fs::remove_file(dir.join("pairing-request"));
    anyhow::bail!("no answer from the agent; is `ywza-agent run` running?")
}
