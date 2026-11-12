//! ywza app backend: keeps control connections to paired devices (and this PC's agent on
//! desktop) and bridges them to the webview.

mod commands;
mod conn;
#[cfg(desktop)]
mod desktop;
mod live;
#[cfg_attr(mobile, path = "mcp_mobile.rs")]
mod mcp;
#[cfg(mobile)]
mod mobile;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(conn::Hub::default())
        .manage(live::Live::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::connection_status,
            commands::connect_device,
            commands::pair_device,
            commands::remove_device,
            commands::send,
            commands::live_channel,
            mcp::mcp_status,
            mcp::mcp_set,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            desktop::setup(app.handle())?;
            #[cfg(mobile)]
            mobile::setup(app.handle())?;
            Ok(())
        });
    #[cfg(desktop)]
    let builder = builder.on_window_event(desktop::on_window_event);
    builder
        .run(tauri::generate_context!())
        .expect("error while running ywza app");
}
