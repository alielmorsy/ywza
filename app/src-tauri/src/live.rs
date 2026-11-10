//! Live view frames: the agent sends each one on its own uni stream (`FrameHeader` frame, then
//! Annex-B). They go to the webview untouched, one channel per device.

use std::{collections::HashMap, sync::Mutex, time::Duration};

use iroh::endpoint::Connection;
use tauri::{
    async_runtime::JoinHandle,
    ipc::{Channel, InvokeResponseBody},
    AppHandle, Manager,
};
use ywza_proto::MAX_FRAME;

/// A stalled stream is skipped so later frames still flow.
const READ_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Default)]
pub struct Live(Mutex<HashMap<String, Channel<InvokeResponseBody>>>);

impl Live {
    /// Replaces the device's sink.
    pub fn set(&self, device: &str, channel: Channel<InvokeResponseBody>) {
        self.0.lock().unwrap().insert(device.into(), channel);
    }

    pub fn clear(&self, device: &str) {
        self.0.lock().unwrap().remove(device);
    }

    /// Drops the sink when the webview side is gone.
    fn push(&self, device: &str, bytes: Vec<u8>) {
        let mut sinks = self.0.lock().unwrap();
        if let Some(ch) = sinks.get(device) {
            if ch.send(InvokeResponseBody::Raw(bytes)).is_err() {
                sinks.remove(device);
            }
        }
    }
}

/// Aborts the forwarder when dropped.
pub struct Forwarder(JoinHandle<()>);

impl Drop for Forwarder {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Reads uni streams one at a time, in the order the agent opened them, so the decoder sees
/// frames in sequence. A reset or stalled stream (stale frame) is skipped.
pub fn forward(app: &AppHandle, conn: Connection, device: &str) -> Forwarder {
    let (app, device) = (app.clone(), device.to_string());
    Forwarder(tauri::async_runtime::spawn(async move {
        while let Ok(mut s) = conn.accept_uni().await {
            let read = tokio::time::timeout(READ_TIMEOUT, s.read_to_end(MAX_FRAME + (32 << 20)));
            if let Ok(Ok(bytes)) = read.await {
                app.state::<Live>().push(&device, bytes);
            }
        }
    }))
}
