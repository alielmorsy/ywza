//! ywza desktop app backend: keeps control connections to this PC's agent and to paired
//! devices and bridges them to the webview.

mod commands;
mod conn;
mod live;
mod mcp;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("window icon").clone())
        .tooltip("ywza")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            tray(app.handle())?;
            conn::start(app.handle(), conn::LOCAL);
            Ok(())
        })
        .on_window_event(|w, e| {
            // Closing hides to the tray; Quit in the tray menu exits.
            if let WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running ywza app");
}
