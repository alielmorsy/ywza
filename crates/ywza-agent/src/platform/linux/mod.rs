mod service;

use tokio::process::Command;

pub fn shell(cmd: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(cmd);
    c
}

pub fn os_description() -> String {
    std::env::consts::OS.into()
}

/// `ywza-agent service [--user <name>]`: the seat0 follower, run as root.
pub fn service(
    args: &[String],
    verbosity: Option<&str>,
    log_file: Option<&std::path::Path>,
) -> Option<anyhow::Result<()>> {
    Some(service::main(args, verbosity, log_file))
}
