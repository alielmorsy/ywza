//! `ywza-agent [-v|-vv|-q] [--log-file <path>] [run|pair|ticket|service]`
//!
//! - `run` (default): the service.
//! - `pair`: asks the running agent for a one-time 6-digit pairing code (via
//!   `<config_dir>/pairing-request`, answered in `<config_dir>/pairing.code`) and prints it.
//!   Valid 10 minutes, single use.
//! - `ticket`: prints `<config_dir>/agent.ticket` (address only; for local clients holding `local-client.key`).
//! - `service [--user <name>]` (Linux, root): keeps a `run` agent on whatever X11 session seat0
//!   shows; see `platform/linux/service`. On Windows the SCM runs it (as LocalSystem) to keep
//!   one on the active console session; see `platform/windows/service`.
//!
//! Logging: `-v` debug, `-vv` trace, `-q` warn beat `[log]` in config.toml; `RUST_LOG` beats both.
//! `run` also writes daily files to `<config_dir>/logs` unless `[log] dir` says otherwise.
//! `--log-file <path>` (any subcommand) appends to that one file instead; so does `[log] file`.
//! `service` passes its own `-v`/`-vv`/`-q`, `--log-file` and `RUST_LOG` on to every agent it starts.

use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use ywza_proto::{Config, LogConfig};

/// Removes the verbosity flags from `args`; the level they ask for, if any.
fn take_verbosity(args: &mut Vec<String>) -> Option<&'static str> {
    let mut level = None;
    args.retain(|a| {
        let l = match a.as_str() {
            "-v" => "debug",
            "-vv" => "trace",
            "-q" => "warn",
            _ => return true,
        };
        level = Some(l);
        false
    });
    level
}

/// Removes `--log-file <path>` or `--log-file=<path>` from `args`; `Err` when the path is missing.
fn take_log_file(args: &mut Vec<String>) -> Result<Option<PathBuf>, ()> {
    let mut file = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--log-file" {
            args.remove(i);
            if i >= args.len() {
                return Err(());
            }
            file = Some(args.remove(i).into());
        } else if let Some(p) = args[i].strip_prefix("--log-file=") {
            file = Some(p.into());
            args.remove(i);
        } else {
            i += 1;
        }
    }
    Ok(file)
}

const USAGE: &str =
    "usage: ywza-agent [-v|-vv|-q] [--log-file <path>] [run|pair|ticket|service [--user <name>]]";

/// `[log]` from the config file for early init; a bad file is reported when the agent loads it.
fn log_config(
    dir: &std::path::Path,
    flag: Option<&str>,
    log_file: Option<PathBuf>,
    file_logs: bool,
) -> LogConfig {
    let mut log = std::fs::read_to_string(dir.join("config.toml"))
        .ok()
        .and_then(|t| toml::from_str::<Config>(&t).ok())
        .map(|c| c.log)
        .unwrap_or_default();
    if let Some(level) = flag {
        log.level = level.into();
    }
    if log_file.is_some() {
        log.file = log_file;
        log.dir = None;
    } else if !file_logs {
        log.dir = None;
        log.file = None;
    } else if log.dir.is_none() {
        log.dir = Some(dir.join("logs"));
    }
    log
}

fn main() -> anyhow::Result<()> {
    // Console attach runs in a child copy of this binary; that copy exits here.
    let mut args: Vec<String> = std::env::args().collect();
    if let Some(code) = ywza_term::helper_main(&args) {
        std::process::exit(code);
    }
    let flag = take_verbosity(&mut args);
    let Ok(log_file) = take_log_file(&mut args) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let dir = ywza_proto::config_dir();
    let cmd = args.get(1).map_or("run", String::as_str).to_string();
    let log = log_config(&dir, flag, log_file.clone(), cmd == "run");
    let _log_guard = ywza_proto::init_log(&log, "ywza-agent", "info").or_else(|e| {
        eprintln!("ywza-agent: {e}; logging to stderr only");
        ywza_proto::init_log(
            &LogConfig {
                dir: None,
                file: None,
                ..log
            },
            "ywza-agent",
            "info",
        )
    })?;
    if args.get(1).map(String::as_str) == Some("service") {
        if let Some(result) = ywza_agent::platform::service(&args, flag, log_file.as_deref()) {
            return result;
        }
    }
    // Before any window/capture call, so every rect is in physical pixels.
    if let Err(e) = ywza_desktop::set_dpi_awareness() {
        tracing::warn!(error = %e, "setting DPI awareness failed");
    }
    let rt = tokio::runtime::Runtime::new()?;
    match cmd.as_str() {
        "run" => rt.block_on(async {
            let agent = ywza_agent::Agent::start(dir, false)
                .await
                .inspect_err(|e| {
                    tracing::error!(error = %format!("{e:#}"), "agent failed to start");
                })?;
            tokio::signal::ctrl_c().await?;
            tracing::info!("shutting down: Ctrl-C");
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
            eprintln!("{USAGE}");
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
