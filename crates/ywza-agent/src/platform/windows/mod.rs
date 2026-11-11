mod os;
mod service;

use tokio::process::Command;

pub use os::os_description;

/// `raw_arg` keeps the user's quoting intact for cmd.exe.
pub fn shell(cmd: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").raw_arg(cmd);
    c
}

/// `ywza-agent service`: the console session follower, run by the SCM.
pub fn service(
    _args: &[String],
    verbosity: Option<&str>,
    log_file: Option<&std::path::Path>,
) -> Option<anyhow::Result<()>> {
    Some(service::main(verbosity, log_file))
}
