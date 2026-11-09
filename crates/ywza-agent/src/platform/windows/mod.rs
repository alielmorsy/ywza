mod os;

use tokio::process::Command;

pub use os::os_description;

/// `raw_arg` keeps the user's quoting intact for cmd.exe.
pub fn shell(cmd: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").raw_arg(cmd);
    c
}

/// `ywza-agent service` exists on Linux only.
pub fn service(
    _args: &[String],
    _verbosity: Option<&str>,
    _log_file: Option<&std::path::Path>,
) -> Option<anyhow::Result<()>> {
    None
}
