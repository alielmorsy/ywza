//! Registers the bundled ywza MCP server with the AI tools on this PC.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

const NAME: &str = "ywza";

#[derive(Serialize)]
pub struct McpClient {
    id: &'static str,
    label: &'static str,
    /// `installed`, `missing`, or `unavailable` when the tool's config is not on this PC.
    state: &'static str,
}

const CLIENTS: [(&str, &str); 3] = [("claude", "Claude Code"), ("codex", "Codex"), ("agy", "Antigravity")];

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The file whose presence says the tool is set up, and where its servers are listed.
fn config(client: &str) -> Option<PathBuf> {
    let h = home()?;
    Some(match client {
        "claude" => h.join(".claude.json"),
        "codex" => std::env::var_os("CODEX_HOME").map_or(h.join(".codex"), PathBuf::from).join("config.toml"),
        _ => h.join(".gemini").join("config").join("mcp_config.json"),
    })
}

fn registered(client: &str, path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else { return false };
    if client == "codex" {
        return text.contains(&format!("[mcp_servers.{NAME}]"));
    }
    serde_json::from_str::<Value>(&text).is_ok_and(|v| v["mcpServers"][NAME].is_object())
}

#[tauri::command]
pub fn mcp_status() -> Vec<McpClient> {
    CLIENTS
        .iter()
        .map(|&(id, label)| {
            let state = match config(id) {
                Some(p) if registered(id, &p) => "installed",
                Some(p) if p.exists() || p.parent().is_some_and(|d| d.exists()) => "missing",
                _ => "unavailable",
            };
            McpClient { id, label, state }
        })
        .collect()
}

/// The bundled server, or the repo's copy when running from a build folder.
fn server(app: &AppHandle) -> Result<PathBuf, String> {
    let bundled = app.path().resource_dir().ok().map(|d| d.join("mcp").join("run.py"));
    let dev = std::env::current_exe()
        .ok()
        .and_then(|e| e.ancestors().map(|a| a.join("plugin").join("server").join("run.py")).find(|p| p.is_file()));
    bundled.filter(|p| p.is_file()).or(dev).ok_or_else(|| "the MCP server files are missing from this install".into())
}

/// Runs a program, trying `<program>.cmd` too on Windows (npm shims). None when it is not installed.
fn run(program: &str, args: &[&str]) -> Option<Result<(), String>> {
    let names = if cfg!(windows) { vec![program.to_string(), format!("{program}.cmd")] } else { vec![program.to_string()] };
    for name in names {
        let mut cmd = Command::new(name);
        cmd.args(args);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        match cmd.output() {
            Ok(o) if o.status.success() => return Some(Ok(())),
            Ok(o) => return Some(Err(String::from_utf8_lossy(&[o.stderr, o.stdout].concat()).trim().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Some(Err(e.to_string())),
        }
    }
    None
}

fn cli(program: &str, args: &[&str]) -> Result<(), String> {
    run(program, args).unwrap_or_else(|| Err(format!("`{program}` is not on PATH")))
}

fn edit_agy(path: &Path, entry: Option<Value>) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut v: Value = serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?;
    let servers = v["mcpServers"].as_object_mut().ok_or("mcpServers is not an object")?;
    match entry {
        Some(e) => servers.insert(NAME.into(), e),
        None => servers.remove(NAME),
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn set(app: &AppHandle, client: &str, install: bool) -> Result<(), String> {
    let path = config(client).ok_or("no home folder")?;
    let remove = |tool: &str| cli(tool, &["mcp", "remove", NAME, "--scope", "user"]);
    if !install {
        return match client {
            "claude" => remove("claude"),
            "codex" => cli("codex", &["mcp", "remove", NAME]),
            _ => edit_agy(&path, None),
        };
    }
    let py = if cfg!(windows) { "python" } else { "python3" };
    if !matches!(run(py, &["--version"]), Some(Ok(()))) {
        return Err("Python 3 is not on PATH. Install it and try again".into());
    }
    let server = server(app)?;
    let server = server.to_string_lossy();
    let server = server.strip_prefix(r"\?\").unwrap_or(&server);
    match client {
        "claude" => {
            let _ = remove("claude");
            cli("claude", &["mcp", "add", "--scope", "user", NAME, "--", py, server])
        }
        "codex" => {
            let _ = run("codex", &["mcp", "remove", NAME]);
            cli("codex", &["mcp", "add", NAME, "--", py, server])
        }
        _ => edit_agy(&path, Some(json!({ "command": py, "args": [server] }))),
    }
}

/// Adds or removes the server in `client`'s own config.
#[tauri::command]
pub async fn mcp_set(app: AppHandle, client: String, install: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set(&app, &client, install)).await.map_err(|e| e.to_string())?
}
