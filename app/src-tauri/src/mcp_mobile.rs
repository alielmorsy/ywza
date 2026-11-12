//! The phone has no AI tools to register with; these keep the command names resolvable.

#[tauri::command]
pub fn mcp_status() -> Vec<String> {
    Vec::new()
}

#[tauri::command]
pub fn mcp_set(_client: String, _install: bool) -> Result<(), String> {
    Err("MCP setup is only available on desktop".into())
}
