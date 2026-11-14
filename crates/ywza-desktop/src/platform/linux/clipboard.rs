//! Clipboard through whichever command-line tool is installed.
// shortcut: shells out instead of owning the X11 selection (that needs an event loop), so it needs xclip, xsel or wl-clipboard; add an in-process owner if installs lack all three.

use super::launch::find_program;
use crate::{Error, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const READERS: &[&[&str]] = &[
    &["xclip", "-selection", "clipboard", "-o"],
    &["xsel", "-b", "-o"],
    &["wl-paste", "--no-newline"],
];
const WRITERS: &[&[&str]] = &[
    &["xclip", "-selection", "clipboard"],
    &["xsel", "-b", "-i"],
    &["wl-copy"],
];

fn tool(list: &'static [&'static [&'static str]]) -> Result<&'static [&'static str]> {
    list.iter()
        .copied()
        .find(|t| find_program(t[0]).is_some())
        .ok_or_else(|| Error::Os("no clipboard tool: install xclip, xsel or wl-clipboard".into()))
}

/// The clipboard's text, None when it holds none (the tools exit non-zero then).
pub fn clipboard_text() -> Result<Option<String>> {
    let t = tool(READERS)?;
    let out = Command::new(t[0])
        .args(&t[1..])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| Error::Os(format!("{}: {e}", t[0])))?;
    Ok(out
        .status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Puts `text` on the clipboard. The tool forks to keep serving the selection.
pub fn set_clipboard_text(text: &str) -> Result<()> {
    let t = tool(WRITERS)?;
    let os = |e: std::io::Error| Error::Os(format!("{}: {e}", t[0]));
    let mut child = Command::new(t[0])
        .args(&t[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(os)?;
    let wrote = child
        .stdin
        .take()
        .map(|mut i| i.write_all(text.as_bytes()))
        .transpose();
    let status = child.wait().map_err(os)?;
    wrote.map_err(os)?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Os(format!("{} failed to set the clipboard", t[0])))
    }
}
