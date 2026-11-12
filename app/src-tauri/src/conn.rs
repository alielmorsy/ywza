//! Per-device control connection: dial, handshake, reconnect, and the bridge to the webview.
//! Messages stay raw JSON both ways, so the app never has to know newer protocol variants.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use iroh::Endpoint;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{async_runtime::JoinHandle, AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use ywza_proto::{
    bind, client, connect, load_or_create_secret_key, read_frame, write_frame, ClientMsg, Control,
    Error, Ticket,
};

use crate::live;

/// This PC's agent; every other device name is a paired device.
pub const LOCAL: &str = "local";
const MIN_RETRY: Duration = Duration::from_secs(2);
const MAX_RETRY: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Serialize, PartialEq)]
pub struct Status {
    pub device: String,
    pub connected: bool,
    /// Why we are not connected (empty when connected).
    pub detail: String,
}

#[derive(Default)]
pub struct Hub {
    tx: Mutex<HashMap<String, UnboundedSender<Value>>>,
    tasks: Mutex<HashMap<String, JoinHandle<()>>>,
    status: Mutex<HashMap<String, Status>>,
    /// Bound endpoints by (key file, relay).
    endpoints: tokio::sync::Mutex<HashMap<(PathBuf, Option<String>), Endpoint>>,
}

impl Hub {
    pub fn send(&self, device: &str, msg: Value) -> Result<(), String> {
        match self.tx.lock().unwrap().get(device) {
            Some(tx) => tx.send(msg).map_err(|_| "not connected".to_string()),
            None => Err("not connected".into()),
        }
    }

    pub fn statuses(&self) -> Vec<Status> {
        self.status.lock().unwrap().values().cloned().collect()
    }
}

/// Starts the connection loop for `name` (no-op when it already runs).
pub fn start(app: &AppHandle, name: &str) {
    let hub = app.state::<Hub>();
    let mut tasks = hub.tasks.lock().unwrap();
    if tasks.contains_key(name) {
        return;
    }
    set_status(app, name, false, "connecting");
    tasks.insert(
        name.into(),
        tauri::async_runtime::spawn(run_loop(app.clone(), name.into())),
    );
}

/// Ends the connection loop and forgets the device; dropping the task closes its connection.
pub fn stop(app: &AppHandle, name: &str) {
    let hub = app.state::<Hub>();
    if let Some(task) = hub.tasks.lock().unwrap().remove(name) {
        task.abort();
    }
    hub.tx.lock().unwrap().remove(name);
    hub.status.lock().unwrap().remove(name);
    app.state::<live::Live>().clear(name);
}

fn set_status(app: &AppHandle, device: &str, connected: bool, detail: impl Into<String>) {
    let s = Status {
        device: device.into(),
        connected,
        detail: detail.into(),
    };
    let hub = app.state::<Hub>();
    let mut all = hub.status.lock().unwrap();
    if all.get(device) != Some(&s) {
        all.insert(device.into(), s.clone());
        let _ = app.emit("connection", s);
    }
}

fn emit_msg(app: &AppHandle, device: &str, msg: Value) {
    let _ = app.emit("agent-msg", json!({ "device": device, "msg": msg }));
}

async fn run_loop(app: AppHandle, device: String) {
    let mut delay = MIN_RETRY;
    let mut last = String::new();
    loop {
        let mut up = false;
        let why = session(&app, &device, &mut up)
            .await
            .err()
            .unwrap_or_default();
        if up {
            delay = MIN_RETRY;
            last.clear();
        }
        if why != last {
            eprintln!("{device} connection: {why}");
            last = why.clone();
        }
        set_status(&app, &device, false, why);
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(MAX_RETRY);
    }
}

struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct CloseOnDrop(iroh::endpoint::Connection);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.close(0u32.into(), b"bye");
    }
}

/// The endpoint for `key` and `relay`, bound on first use.
async fn endpoint(app: &AppHandle, key: &Path, relay: Option<String>) -> Result<Endpoint, String> {
    let hub = app.state::<Hub>();
    let mut eps = hub.endpoints.lock().await;
    let slot = (key.to_path_buf(), relay);
    if let Some(ep) = eps.get(&slot) {
        return Ok(ep.clone());
    }
    let secret = load_or_create_secret_key(key).map_err(|e| e.to_string())?;
    let ep = bind(secret, slot.1.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    eps.insert(slot, ep.clone());
    Ok(ep)
}

/// The endpoint to dial `device` from, and its ticket.
async fn target(app: &AppHandle, device: &str) -> Result<(Endpoint, Ticket), String> {
    let name = (device != LOCAL).then_some(device);
    let (key, ticket, relay) = client::resolve(name).map_err(|e| format!("{e:#}"))?;
    // The agent creates and trusts the local key; a key we minted would not be trusted.
    if name.is_none() && !key.exists() {
        return Err(format!("agent not set up ({} missing)", key.display()));
    }
    Ok((endpoint(app, &key, relay).await?, ticket))
}

fn client_name(device: &str) -> String {
    if device == LOCAL {
        return "ywza-app".into();
    }
    if cfg!(mobile) {
        return "phone".into();
    }
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "ywza-app".into())
}

/// Dials and sends Hello. The Welcome is forwarded to the webview, which needs its fields.
async fn handshake(
    ep: &Endpoint,
    ticket: &Ticket,
    device: &str,
) -> Result<(Control, Value), String> {
    let mut c = connect(ep, ticket).await.map_err(|e| e.to_string())?;
    let hello = ClientMsg::Hello {
        client_name: client_name(device),
        token: None,
    };
    write_frame(&mut c.send, &hello)
        .await
        .map_err(|e| e.to_string())?;
    let first: Value = read_frame(&mut c.recv).await.map_err(|e| e.to_string())?;
    match first["type"].as_str() {
        Some("welcome") => Ok((c, first)),
        Some("error") => Err(first["message"].as_str().unwrap_or("refused").to_string()),
        _ => Err(format!("expected welcome, got {first}")),
    }
}

/// One connection attempt; returns why it ended.
async fn session(app: &AppHandle, device: &str, up: &mut bool) -> Result<(), String> {
    let (ep, ticket) = target(app, device).await?;
    let (
        Control {
            conn,
            mut send,
            mut recv,
        },
        welcome,
    ) = tokio::time::timeout(CONNECT_TIMEOUT, handshake(&ep, &ticket, device))
        .await
        .map_err(|_| "connect timed out".to_string())??;
    let _close = CloseOnDrop(conn.clone());

    let (tx, mut rx) = unbounded_channel::<Value>();
    app.state::<Hub>()
        .tx
        .lock()
        .unwrap()
        .insert(device.into(), tx);
    emit_msg(app, device, welcome);
    set_status(app, device, true, "");
    *up = true;

    let _uni = live::forward(app, conn.clone(), device);
    let (reader_app, reader_dev) = (app.clone(), device.to_string());
    let mut reader = AbortOnDrop(tauri::async_runtime::spawn(async move {
        loop {
            match read_frame::<_, Value>(&mut recv).await {
                Ok(m) => emit_msg(&reader_app, &reader_dev, m),
                Err(Error::Json(e)) => eprintln!("{reader_dev}: skipping undecodable frame: {e}"),
                Err(e) => break e.to_string(),
            }
        }
    }));

    let end = loop {
        tokio::select! {
            r = &mut reader.0 => break r.unwrap_or_else(|e| e.to_string()),
            m = rx.recv() => match m {
                Some(m) => if let Err(e) = write_frame(&mut send, &m).await { break e.to_string() },
                None => break "channel closed".into(),
            },
        }
    };
    app.state::<Hub>().tx.lock().unwrap().remove(device);
    // "connection lost" alone hides why (timed out, reset, closed by the agent, ...).
    Err(match conn.close_reason() {
        Some(why) => format!("{end} ({why})"),
        None => end,
    })
}
