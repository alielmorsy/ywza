//! File transfer from the app to a paired device, over the endpoint its control connection uses.

use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use tauri::{
    ipc::{InvokeBody, Request},
    AppHandle, Manager,
};
use ywza_proto::client::{self, copy::Stats};

use crate::conn::{self, LOCAL};

const DEFAULT_DEST: &str = "Downloads/";

/// Sends a local file or folder to `dest` on `device` (default `Downloads`).
#[tauri::command]
pub async fn push_file(app: AppHandle, device: String, path: String, dest: String) -> Result<String, String> {
    push(&app, &device, Path::new(&path), &dest).await
}

/// Sends the bytes of a file the webview picked. Headers `x-device`, `x-name` and `x-dest` are
/// percent-encoded. Android has no raw bodies: the bytes arrive as a JSON array of numbers.
#[tauri::command]
pub async fn push_bytes(app: AppHandle, request: Request<'_>) -> Result<String, String> {
    let header = |k: &str| {
        request
            .headers()
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(unescape)
            .unwrap_or_default()
    };
    let (device, name, dest) = (header("x-device"), header("x-name"), header("x-dest"));
    let name = Path::new(&name)
        .file_name()
        .ok_or("missing file name")?
        .to_owned();
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("xfer")
        .join(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
                .to_string(),
        );
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(name);
    let written = match request.body() {
        InvokeBody::Raw(b) => std::fs::write(&file, b).map_err(|e| e.to_string()),
        InvokeBody::Json(Value::Array(a)) => a
            .iter()
            .map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| "bad file data".to_string())
            .and_then(|b| std::fs::write(&file, b).map_err(|e| e.to_string())),
        _ => Err("no file data".into()),
    };
    let r = match written {
        Ok(()) => push(&app, &device, &file, &dest).await,
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_dir_all(&dir);
    r
}

async fn push(app: &AppHandle, device: &str, path: &Path, dest: &str) -> Result<String, String> {
    if device == LOCAL {
        return Err("pick a paired device".into());
    }
    let dest = if dest.trim().is_empty() { DEFAULT_DEST } else { dest.trim() };
    let ep = conn::device_endpoint(app, device).await?;
    let st = client::copy::push(&ep, device, path, dest, path.is_dir())
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(summary(&st, device, dest))
}

fn summary(st: &Stats, device: &str, dest: &str) -> String {
    let plural = if st.files == 1 { "" } else { "s" };
    let mut s = format!("sent {} file{plural}, {} to {device}:{dest}", st.files, size(st.bytes));
    for e in &st.errors {
        s.push('\n');
        s.push_str(e);
    }
    s
}

fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let (mut v, mut i) = (bytes as f64, 0);
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[i]) }
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%')
            .then(|| s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
