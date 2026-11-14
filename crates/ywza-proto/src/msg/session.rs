use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::ScreenRect;

/// Conversation identifier; a UUID rendered as a string on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub status: SessionStatus,
    /// Last activity, Unix milliseconds.
    pub updated_ms: u64,
    /// The paired device this session operates with our AI; absent: this machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Other paired devices the session's AI may use besides this machine.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
}

/// One entry in a session's history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        args: Value,
        /// The device the call ran on; absent: the machine running the session's tools.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device: Option<String>,
    },
    ToolResult {
        id: String,
        ok: bool,
        text: String,
        has_image: bool,
        /// PNG (base64) shown to the user only (`show_screenshot`); the model never gets it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image: Option<String>,
    },
    Done {
        summary: String,
    },
    Failed {
        error: String,
    },
    /// Token receipt, once per run just before done/failed. `screenshot_estimate`: input plus
    /// output tokens a screenshot-based agent would have used for the same steps. Costs need prices.
    Usage {
        input: u64,
        output: u64,
        cached: u64,
        requests: u32,
        tool_calls: u32,
        screenshot_estimate: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cost: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screenshot_cost: Option<f64>,
    },
    /// The run paused because the AI needs the user (`kind`: otp, uac, credentials, captcha,
    /// login or stuck) until a `Resume` arrives. `window` is `w<id>`; `rect` is the window's
    /// rect relative to `monitor`.
    Help {
        kind: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<String>,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        monitor: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<ScreenRect>,
    },
    /// The user handed back; `digest` is what the AI was told about their changes.
    Resumed {
        digest: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::testutil::check;
    use serde_json::json;

    #[test]
    fn session_events() {
        use SessionEvent::*;
        check(User { text: "u".into() }, json!({"type":"user","text":"u"}));
        check(
            Assistant { text: "a".into() },
            json!({"type":"assistant","text":"a"}),
        );
        check(
            ToolCall {
                id: "c1".into(),
                name: "click".into(),
                args: json!({"element":1}),
                device: None,
            },
            json!({"type":"tool_call","id":"c1","name":"click","args":{"element":1}}),
        );
        check(
            ToolCall {
                id: "c2".into(),
                name: "click".into(),
                args: json!({}),
                device: Some("vm".into()),
            },
            json!({"type":"tool_call","id":"c2","name":"click","args":{},"device":"vm"}),
        );
        check(
            ToolResult {
                id: "c1".into(),
                ok: true,
                text: "ok".into(),
                has_image: false,
                image: None,
            },
            json!({"type":"tool_result","id":"c1","ok":true,"text":"ok","has_image":false}),
        );
        check(
            Done {
                summary: "s".into(),
            },
            json!({"type":"done","summary":"s"}),
        );
        check(
            Failed { error: "e".into() },
            json!({"type":"failed","error":"e"}),
        );
        check(
            Usage {
                input: 10,
                output: 2,
                cached: 4,
                requests: 1,
                tool_calls: 0,
                screenshot_estimate: 12,
                cost: Some(0.5),
                screenshot_cost: None,
            },
            json!({"type":"usage","input":10,"output":2,"cached":4,"requests":1,
                "tool_calls":0,"screenshot_estimate":12,"cost":0.5}),
        );
        check(
            Help {
                kind: "otp".into(),
                reason: "r".into(),
                window: Some("w123".into()),
                title: "Sign in".into(),
                monitor: Some(0),
                rect: Some(ScreenRect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                }),
            },
            json!({"type":"help","kind":"otp","reason":"r","window":"w123","title":"Sign in",
                "monitor":0,"rect":{"x":1,"y":2,"width":3,"height":4}}),
        );
        check(
            Help {
                kind: "stuck".into(),
                reason: "r".into(),
                window: None,
                title: String::new(),
                monitor: None,
                rect: None,
            },
            json!({"type":"help","kind":"stuck","reason":"r"}),
        );
        check(
            Resumed { digest: "d".into() },
            json!({"type":"resumed","digest":"d"}),
        );
    }
}
