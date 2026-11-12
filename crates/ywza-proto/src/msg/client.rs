use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ApprovalMode, Config, InputEvent, SessionId, Target};

/// Client to agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello {
        client_name: String,
        /// Local TCP clients authenticate with the token from `local-tcp.json`.
        #[serde(
            default,
            alias = "pairing_code",
            skip_serializing_if = "Option::is_none"
        )]
        token: Option<String>,
    },
    /// Starts a new session when `session` is absent; an id the agent does not know creates a
    /// session with that id. `device` runs our AI on a paired device: every tool call goes to it.
    /// `devices` keeps the AI here and lets it use those other paired devices per call, through a
    /// `device` argument on every tool. Peers need `drive` for either. `model` and `effort`
    /// override the configured ones for this run (local clients only).
    /// `approval` (local clients only) is sent by the desktop app: it gates the session's tool
    /// calls and is kept for the session. Without it, tools run with no approval step, except
    /// that a peer's session on devices always follows the approval rules.
    Prompt {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<SessionId>,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        devices: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        approval: Option<ApprovalMode>,
    },
    /// Answers an `ApprovalRequest`. `remember` with `allow` always allows the tool from now on
    /// (local clients and peers with `settings` only; for others it is a plain allow).
    Approve {
        request: u64,
        allow: bool,
        #[serde(default)]
        remember: bool,
    },
    Cancel {
        session: SessionId,
    },
    /// With `max_width`, the PNG is downscaled to at most that width.
    Screenshot {
        target: Target,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_width: Option<u32>,
    },
    GetConfig,
    SetConfig {
        config: Config,
    },
    ListSessions,
    GetSession {
        session: SessionId,
    },
    LiveStart {
        monitor: u32,
    },
    LiveStop,
    Input {
        event: InputEvent,
    },
    /// Asks for a one-time pairing code (answered with `PairingCode`).
    CreatePairing,
    /// The agent's AI tools (answered with `Tools`), for driving them with your own AI.
    ListTools,
    /// Runs one tool directly, with no approval step. Answered with `ToolResult` carrying the
    /// same `call`.
    CallTool {
        call: u64,
        name: String,
        #[serde(default)]
        args: Value,
    },
    /// The monitors (answered with `Monitors`).
    ListMonitors,
    /// Model ids of the configured LLM endpoint (answered with `Models`; local clients only).
    ListModels,
    /// This agent's paired devices (answered with `Devices`; peers need `drive`).
    ListDevices,
    /// A window's title, monitor and rect, and with `elements` its UI elements (answered with
    /// `WindowInfo`). `window` is `w<id>` as in `list_windows`. The elements are the window's
    /// latest `get_elements` list (same ids); a fresh one is taken if there is none yet.
    WindowInfo {
        window: String,
        #[serde(default)]
        elements: bool,
    },
    /// Sends `path` (relative and `~` paths are under the agent user's home): `XferStart`,
    /// one uni stream per entry, then `XferDone`.
    Pull {
        xfer: u64,
        path: String,
        #[serde(default)]
        recursive: bool,
    },
    /// Readies `dest` for the entry streams of `xfer` (`name` is the source's file name),
    /// answered with `XferReady`.
    Push {
        xfer: u64,
        dest: String,
        name: String,
    },
    /// The client opened all `files` streams of a push; `XferDone` follows once they are written.
    PushEnd {
        xfer: u64,
        files: u64,
    },
}

impl ClientMsg {
    /// The `type` tag, for logs (the payload can hold typed text).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::Prompt { .. } => "prompt",
            Self::Approve { .. } => "approve",
            Self::Cancel { .. } => "cancel",
            Self::Screenshot { .. } => "screenshot",
            Self::GetConfig => "get_config",
            Self::SetConfig { .. } => "set_config",
            Self::ListSessions => "list_sessions",
            Self::GetSession { .. } => "get_session",
            Self::LiveStart { .. } => "live_start",
            Self::LiveStop => "live_stop",
            Self::Input { .. } => "input",
            Self::CreatePairing => "create_pairing",
            Self::ListTools => "list_tools",
            Self::CallTool { .. } => "call_tool",
            Self::ListMonitors => "list_monitors",
            Self::ListModels => "list_models",
            Self::ListDevices => "list_devices",
            Self::WindowInfo { .. } => "window_info",
            Self::Pull { .. } => "pull",
            Self::Push { .. } => "push",
            Self::PushEnd { .. } => "push_end",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::testutil::{check, sid, SID};
    use serde_json::json;

    #[test]
    fn client_msgs() {
        use ClientMsg::*;
        check(
            Hello {
                client_name: "cli".into(),
                token: None,
            },
            json!({"type":"hello","client_name":"cli"}),
        );
        check(
            Prompt {
                session: None,
                text: "hi".into(),
                device: None,
                devices: Vec::new(),
                model: None,
                effort: None,
                approval: None,
            },
            json!({"type":"prompt","text":"hi"}),
        );
        check(
            Prompt {
                session: Some(sid()),
                text: "hi".into(),
                device: Some("vm".into()),
                devices: vec!["pc".into()],
                model: Some("m".into()),
                effort: Some("high".into()),
                approval: Some(ApprovalMode::Always),
            },
            json!({"type":"prompt","session":SID,"text":"hi","device":"vm","devices":["pc"],"model":"m",
                   "effort":"high","approval":"always"}),
        );
        check(
            Approve {
                request: 7,
                allow: true,
                remember: true,
            },
            json!({"type":"approve","request":7,"allow":true,"remember":true}),
        );
        assert_eq!(
            serde_json::from_value::<ClientMsg>(
                json!({"type":"approve","request":7,"allow":false})
            )
            .unwrap(),
            Approve {
                request: 7,
                allow: false,
                remember: false
            }
        );
        check(ListMonitors, json!({"type":"list_monitors"}));
        check(ListModels, json!({"type":"list_models"}));
        check(ListDevices, json!({"type":"list_devices"}));
        check(
            WindowInfo {
                window: "w3".into(),
                elements: true,
            },
            json!({"type":"window_info","window":"w3","elements":true}),
        );
        check(
            Cancel { session: sid() },
            json!({"type":"cancel","session":SID}),
        );
        check(
            Screenshot {
                target: Target::Monitor { id: 2 },
                max_width: Some(640),
            },
            json!({"type":"screenshot","target":{"type":"monitor","id":2},"max_width":640}),
        );
        check(
            Screenshot {
                target: Target::Window { id: 3 },
                max_width: None,
            },
            json!({"type":"screenshot","target":{"type":"window","id":3}}),
        );
        check(GetConfig, json!({"type":"get_config"}));
        check(
            SetConfig {
                config: Config::default(),
            },
            json!({"type":"set_config","config":serde_json::to_value(Config::default()).unwrap()}),
        );
        check(ListSessions, json!({"type":"list_sessions"}));
        check(
            GetSession { session: sid() },
            json!({"type":"get_session","session":SID}),
        );
        check(
            LiveStart { monitor: 1 },
            json!({"type":"live_start","monitor":1}),
        );
        check(LiveStop, json!({"type":"live_stop"}));
        check(CreatePairing, json!({"type":"create_pairing"}));
        check(ListTools, json!({"type":"list_tools"}));
        check(
            CallTool {
                call: 4,
                name: "wait".into(),
                args: json!({"ms":1}),
            },
            json!({"type":"call_tool","call":4,"name":"wait","args":{"ms":1}}),
        );
        assert_eq!(
            serde_json::from_value::<ClientMsg>(
                json!({"type":"call_tool","call":4,"name":"wait","args":{}})
            )
            .unwrap(),
            CallTool {
                call: 4,
                name: "wait".into(),
                args: json!({}),
            }
        );
        check(
            Pull {
                xfer: 2,
                path: "~/d".into(),
                recursive: true,
            },
            json!({"type":"pull","xfer":2,"path":"~/d","recursive":true}),
        );
        check(
            Push {
                xfer: 2,
                dest: "/tmp".into(),
                name: "d".into(),
            },
            json!({"type":"push","xfer":2,"dest":"/tmp","name":"d"}),
        );
        check(
            PushEnd { xfer: 2, files: 3 },
            json!({"type":"push_end","xfer":2,"files":3}),
        );
        check(
            Input {
                event: InputEvent::Key {
                    key: "a".into(),
                    down: true,
                },
            },
            json!({"type":"input","event":{"type":"key","key":"a","down":true}}),
        );
    }
}
