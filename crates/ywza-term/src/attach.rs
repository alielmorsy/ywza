//! Existing consoles: types, plus read/write through the helper child process.

use crate::helper::{call, Request};
use crate::keys::parse_keys;
use crate::{platform, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    WindowsTerminal,
    /// Classic conhost window.
    Conhost,
    /// Other pseudo-console host (VS Code, ssh, ...) or no window found.
    Other,
}

#[derive(Debug, Clone)]
pub struct Attachable {
    /// Process to attach to. For a console shared by several processes this is the root one.
    pub pid: u32,
    pub name: String,
    pub host: Host,
    /// Terminal window (`HWND`) when it could be mapped.
    pub hwnd: Option<isize>,
    /// Console title (the tab title in Windows Terminal).
    pub title: String,
    /// Parent chain of `pid`, nearest first.
    pub ancestors: Vec<u32>,
    /// Set when the console cannot be attached (e.g. elevated).
    pub error: Option<String>,
}

/// Console processes (shells and anything running directly under a console host), one entry
/// per console, mapped to their window where possible. Empty on non-Windows.
pub fn list_attachable() -> Result<Vec<Attachable>> {
    platform::list_attachable()
}

/// Last `lines` lines of the console's screen buffer.
pub fn attached_read(pid: u32, lines: usize) -> Result<String> {
    Ok(call(&Request::Read { pid, lines })?
        .text
        .unwrap_or_default())
}

pub fn attached_write(pid: u32, text: &str, enter: bool) -> Result<()> {
    call(&Request::Write {
        pid,
        text: text.to_string(),
        enter,
    })
    .map(|_| ())
}

/// `keys` is a whitespace-separated list such as `"esc"`, `"ctrl+c"`, `"up up enter"`.
pub fn attached_keys(pid: u32, keys: &str) -> Result<()> {
    parse_keys(keys)?;
    call(&Request::Keys {
        pid,
        keys: keys.to_string(),
    })
    .map(|_| ())
}
