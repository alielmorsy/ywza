use super::protocol::{Request, Response, HELPER_ARG};
use crate::{err, platform, Error, Result};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);

static HELPER: OnceLock<PathBuf> = OnceLock::new();

/// Binary to run as the helper. Defaults to the current executable, which must call
/// `helper_main` first thing in `main()`. Tests use the bundled `ywza-term-helper` binary.
pub fn set_helper_path(p: impl Into<PathBuf>) {
    let _ = HELPER.set(p.into());
}

/// Run one request in a helper child process.
pub(crate) fn call(req: &Request) -> Result<Response> {
    let exe = match HELPER.get() {
        Some(p) => p.clone(),
        None => std::env::current_exe().map_err(err)?,
    };
    let mut cmd = Command::new(exe);
    cmd.arg(HELPER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    platform::hide_window(&mut cmd);
    let mut child = cmd.spawn().map_err(err)?;
    let body = serde_json::to_vec(req).map_err(err)?;
    child
        .stdin
        .take()
        .ok_or_else(|| err("no stdin"))?
        .write_all(&body)
        .map_err(err)?;
    let mut stdout = child.stdout.take().ok_or_else(|| err("no stdout"))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        let _ = tx.send(s);
    });
    let out = match rx.recv_timeout(TIMEOUT) {
        Ok(s) => s,
        Err(_) => {
            let _ = child.kill();
            return Err(err("console helper timed out"));
        }
    };
    let _ = child.wait();
    let resp: Response = serde_json::from_str(out.trim())
        .map_err(|e| Error(format!("bad helper reply ({e}): {out:?}")))?;
    match resp.error {
        Some(e) => Err(Error(e)),
        None => Ok(resp),
    }
}
