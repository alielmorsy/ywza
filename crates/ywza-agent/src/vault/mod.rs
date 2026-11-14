//! Secrets the model never sees: it types `{{vault:key}}`, the agent swaps in the value from
//! `<dir>/vault.toml` (flat `key = "value"`) only when input is sent, and replaces every value
//! in tool output with the placeholder again.

mod commands;
mod scrub;
mod store;
mod substitute;

use std::{path::{Path, PathBuf}, sync::OnceLock};

use serde_json::Value;

pub use commands::{cli, USAGE};

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Sets the agent dir the vault lives in; the first call wins.
pub fn init(dir: &Path) {
    let _ = DIR.set(dir.to_path_buf());
}

/// Read on every use, so edits to the file apply without a restart.
fn entries() -> anyhow::Result<store::Entries> {
    DIR.get().map_or_else(|| Ok(Default::default()), |d| store::load(d))
}

/// `args` for tool `name` with `{{vault:key}}` replaced where the tool types text.
pub fn substitute(name: &str, args: &Value) -> Result<Value, String> {
    if !args.to_string().contains(substitute::OPEN) {
        return Ok(args.clone());
    }
    let entries = entries().map_err(|e| format!("vault unavailable: {e:#}"))?;
    substitute::substitute_args(name, args, &entries)
}

/// Puts the placeholder back wherever `text` holds a vault value.
pub fn scrub(text: &mut String) {
    if let Ok(entries) = entries() {
        if !entries.is_empty() {
            *text = scrub::scrub_text(text, &entries);
        }
    }
}

/// The vault values long enough to hide, for screenshot blurring.
pub fn secrets() -> Vec<String> {
    let entries = entries().unwrap_or_default();
    scrub::secrets(&entries).into_iter().map(|(_, v)| v.to_string()).collect()
}
