//! `ywza`: command-line client for the ywza agent.

mod cmd;
mod render;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ywza_proto::{client, SessionId};

use cmd::config::ConfigCmd;

const CLIENT_NAME: &str = "ywza-cli";

#[derive(Parser)]
#[command(name = "ywza", version)]
struct Cli {
    /// Talk to this paired device (see `ywza devices`) instead of the local agent.
    #[arg(short, long, global = true, value_parser = parse_device)]
    device: Option<String>,
    /// Show full tool results.
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Diagnostics on stderr: error, warn, info, debug or trace. YWZA_LOG, then RUST_LOG, override it.
    #[arg(long, global = true, default_value = "warn")]
    log: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Pair with another machine's agent using the 6-digit code from `ywza-agent pair` there.
    Pair {
        #[arg(value_parser = parse_code)]
        code: String,
        /// Name to reach it by with --device.
        #[arg(long, default_value = "remote", value_parser = parse_device)]
        name: String,
    },
    /// List paired devices.
    Devices,
    /// Send a prompt and stream the session until it finishes.
    Prompt {
        text: String,
        #[arg(long, value_parser = parse_sid)]
        session: Option<SessionId>,
    },
    /// List sessions.
    Sessions,
    /// Print a session's history.
    Show {
        #[arg(value_parser = parse_sid)]
        session: SessionId,
    },
    /// Save a screenshot as PNG (default: monitor 0).
    Screenshot {
        #[arg(long, conflicts_with = "window")]
        monitor: Option<u32>,
        #[arg(long)]
        window: Option<u64>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Open a window that shows a monitor live.
    Live {
        #[arg(long, default_value_t = 0)]
        monitor: u32,
    },
    /// Copy files and folders: `ywza cp -r vm:/home/vm/folder E:\files`. A `device:` prefix
    /// names a paired device; a bare path is on this machine. The last path is the destination.
    Cp {
        /// Copy folders and everything in them.
        #[arg(short, long)]
        recursive: bool,
        #[arg(required = true, num_args = 2..)]
        paths: Vec<String>,
    },
    #[command(subcommand)]
    Config(ConfigCmd),
    /// After the handshake, pipe raw protocol frames between stdin/stdout and the agent
    /// (used by the Python MCP server for paired devices).
    Bridge,
}

/// Device names become file names (`devices/<name>.json`).
fn parse_device(s: &str) -> Result<String, String> {
    client::valid_device_name(s)
        .then(|| s.to_string())
        .ok_or_else(|| "use letters, digits, - and _ (\"local\" means this machine)".into())
}

fn parse_code(s: &str) -> Result<String, String> {
    let ok = s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit());
    ok.then(|| s.to_string())
        .ok_or_else(|| "the pairing code is 6 digits".into())
}

fn parse_sid(s: &str) -> Result<SessionId, String> {
    s.parse().map(SessionId).map_err(|e| format!("{e}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    // Stderr only: `bridge` speaks its protocol on stdout.
    let level = std::env::var("YWZA_LOG").unwrap_or_else(|_| cli.log.clone());
    let log = ywza_proto::LogConfig {
        level,
        ..Default::default()
    };
    ywza_proto::init_log(&log, "ywza", "warn")?;
    match &cli.cmd {
        Cmd::Devices => return cmd::devices::run(),
        Cmd::Pair { code, name } => return cmd::pair::run(code, name).await,
        Cmd::Cp { recursive, paths } => return cmd::cp::run(paths, *recursive).await,
        _ => {}
    }

    let (ep, mut c, _) = match cli.device.as_deref() {
        Some(name) => client::open_device(name, CLIENT_NAME).await?,
        None => client::open_local(CLIENT_NAME).await?,
    };
    let bridge = matches!(cli.cmd, Cmd::Bridge);
    let r = match cli.cmd {
        Cmd::Bridge => cmd::bridge::run(&mut c).await,
        Cmd::Prompt { text, session } => cmd::prompt::run(&mut c, text, session, cli.verbose).await,
        Cmd::Sessions => cmd::sessions::run(&mut c).await,
        Cmd::Show { session } => cmd::show::run(&mut c, session, cli.verbose).await,
        Cmd::Screenshot {
            monitor,
            window,
            output,
        } => cmd::screenshot::run(&mut c, monitor, window, output).await,
        Cmd::Live { monitor } => cmd::live::run(&mut c, cli.device.as_deref(), monitor).await,
        Cmd::Config(sub) => cmd::config::run(&mut c, sub).await,
        Cmd::Pair { .. } | Cmd::Devices | Cmd::Cp { .. } => unreachable!(),
    };
    client::close(ep, &c).await;
    // Exit right here: tokio's stdin reader thread would block runtime shutdown until the
    // client closes our stdin, so the client would never see EOF when the agent drops the
    // link (e.g. a login-screen switch) and its call would hang.
    if bridge {
        std::process::exit(r.is_ok() as i32 ^ 1);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codes_and_device_names() {
        assert!(parse_code("012345").is_ok());
        assert!(parse_code("12345").is_err());
        assert!(parse_code("12345a").is_err());
        assert!(parse_device("linux-vm_1").is_ok());
        assert!(parse_device("local").is_err());
        assert!(parse_device("a/b").is_err());
    }
}
