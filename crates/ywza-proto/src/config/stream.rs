use serde::{Deserialize, Serialize};

/// Live view encoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stream {
    pub fps: u32,
    /// 0 adapts the bitrate to the connection.
    pub bitrate_kbps: u32,
    /// Draw the cursor into the video. Off: its shape is sent as `AgentMsg::Cursor` and the
    /// client shows it on its own pointer (no second, lagging cursor).
    pub cursor: bool,
}

impl Default for Stream {
    fn default() -> Self {
        Self {
            fps: 30,
            bitrate_kbps: 0,
            cursor: false,
        }
    }
}
