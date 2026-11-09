use tokio::process::Command;

pub fn shell(cmd: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(cmd);
    c
}

pub fn os_description() -> String {
    std::env::consts::OS.into()
}

/// `ywza-agent service` exists on Linux only.
pub fn service(
    _args: &[String],
    _verbosity: Option<&str>,
    _log_file: Option<&std::path::Path>,
) -> Option<anyhow::Result<()>> {
    None
}
