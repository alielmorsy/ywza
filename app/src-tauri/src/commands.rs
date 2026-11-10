//! Tauri commands: the contract in `app/src/api.ts`.

use serde_json::Value;
use tauri::{
    ipc::{Channel, InvokeResponseBody},
    AppHandle, State,
};
use ywza_proto::client;

use crate::{
    conn::{self, Hub, Status, LOCAL},
    live::Live,
};

/// Paired devices, sorted; never "local".
#[tauri::command]
pub fn list_devices() -> Vec<String> {
    let mut names: Vec<String> = client::list_devices()
        .into_iter()
        .filter(|n| n != LOCAL)
        .collect();
    names.sort();
    names
}

#[tauri::command]
pub fn connection_status(hub: State<'_, Hub>) -> Vec<Status> {
    hub.statuses()
}

/// Starts the connection loop of a paired device (no-op when it already runs).
#[tauri::command]
pub fn connect_device(app: AppHandle, name: String) -> Result<(), String> {
    if name == LOCAL {
        return Ok(());
    }
    if !list_devices().contains(&name) {
        return Err(format!("no paired device `{name}`"));
    }
    conn::start(&app, &name);
    Ok(())
}

/// Pairs with the device showing `code` and stores it as `name`. The caller connects afterwards.
#[tauri::command]
pub async fn pair_device(name: String, code: String) -> Result<(), String> {
    let name = name.trim();
    if !client::valid_device_name(name) || name.eq_ignore_ascii_case(LOCAL) {
        return Err("name: use letters, digits, - and _".into());
    }
    if list_devices().iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Err(format!("a device named `{name}` already exists"));
    }
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err("the code is 6 digits".into());
    }
    client::pair(code, name).await.map_err(|e| format!("{e:#}"))
}

/// Closes the connection and deletes the device file.
#[tauri::command]
pub fn remove_device(app: AppHandle, name: String) -> Result<(), String> {
    if name == LOCAL {
        return Err("this PC cannot be removed".into());
    }
    conn::stop(&app, &name);
    client::remove_device(&name).map_err(|e| format!("{e:#}"))
}

/// Writes `msg` (a `ClientMsg`) to the device's control stream.
#[tauri::command]
pub fn send(hub: State<'_, Hub>, device: String, msg: Value) -> Result<(), String> {
    hub.send(&device, msg)
}

/// Registers the channel that receives `device`'s live frames, replacing an earlier one.
#[tauri::command]
pub fn live_channel(live: State<'_, Live>, device: String, channel: Channel<InvokeResponseBody>) {
    live.set(&device, channel);
}
