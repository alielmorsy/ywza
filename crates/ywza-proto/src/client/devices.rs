use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use iroh::EndpointAddr;

use crate::{config_dir, PairInfo, Ticket};

pub fn devices_dir() -> PathBuf {
    config_dir().join("devices")
}

pub fn device_file(name: &str) -> PathBuf {
    devices_dir().join(format!("{name}.json"))
}

/// Device names become file names, and `local` means this machine.
pub fn valid_device_name(s: &str) -> bool {
    !s.is_empty()
        && s != "local"
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Paired device names, sorted.
pub fn list_devices() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(devices_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(String::from)
        })
        .collect();
    names.sort();
    names
}

pub fn remove_device(name: &str) -> Result<()> {
    if !valid_device_name(name) {
        bail!("bad device name `{name}`");
    }
    std::fs::remove_file(device_file(name)).with_context(|| format!("no paired device `{name}`"))
}

/// `relay_url` from the local `config.toml`; a missing file means the public relays.
pub fn relay_url() -> Result<Option<String>> {
    let path = config_dir().join("config.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(toml::from_str::<crate::Config>(&text)
            .with_context(|| format!("parsing {}", path.display()))?
            .relay_url),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// The device's ticket and the relay it was paired with, if any.
pub fn load_device(name: &str) -> Result<(Ticket, Option<String>)> {
    let file = device_file(name);
    tracing::debug!(device = name, file = %file.display(), "resolving the paired device");
    let text = std::fs::read_to_string(&file)
        .with_context(|| format!("no paired device `{name}` (see `ywza devices`, `ywza pair`)"))?;
    let info: PairInfo =
        serde_json::from_str(&text).with_context(|| format!("device `{name}` file"))?;
    let addr = info.addr.unwrap_or_else(|| EndpointAddr::new(info.node_id));
    Ok((Ticket { addr }, info.relay))
}

pub(super) fn load_local() -> Result<Ticket> {
    let path = config_dir().join("agent.ticket");
    tracing::debug!(file = %path.display(), "resolving the local agent");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no {} (has ywza-agent run here?)", path.display()))?;
    Ok(text.parse()?)
}
