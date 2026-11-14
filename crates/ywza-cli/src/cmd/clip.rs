//! `ywza clip`: print a device's clipboard text, or copy it from one device to another.

use anyhow::{bail, Result};
use serde_json::json;

use super::remote::call;

/// Well under the frame limit; the tool's own cap would cut the text to what suits a model.
const MAX_CHARS: u64 = 4_000_000;

pub async fn run(from: &str, to: Option<&str>) -> Result<()> {
    let got = call(from, "clipboard_get", json!({ "max_chars": MAX_CHARS })).await?;
    if !got.ok {
        bail!("{from}: {}", got.text);
    }
    let Some(to) = to else {
        println!("{}", got.text);
        return Ok(());
    };
    let n = got.text.chars().count();
    let set = call(to, "clipboard_set", json!({ "text": got.text })).await?;
    if !set.ok {
        bail!("{to}: {}", set.text);
    }
    println!("copied {n} chars from {from} to {to}");
    Ok(())
}
