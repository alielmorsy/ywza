//! Phone setup: no local agent, so device files and the client key live in app storage.

use std::fs;

use tauri::{AppHandle, Manager};

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    std::env::set_var("YWZA_CONFIG_DIR", dir);
    app.plugin(tauri_plugin_barcode_scanner::init())?;
    Ok(())
}
