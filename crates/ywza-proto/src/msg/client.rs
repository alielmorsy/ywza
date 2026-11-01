use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Config, InputEvent, SessionId, Target};

/// Client to agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello {
        client_name: String,
        /// Local TCP clients authenticate with the token from `local-tcp.json`.
        #[serde(default, alias = "pairing_code", skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// Starts a new session when `session` is absent.
    Prompt {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<SessionId>,
        text: String,
    },
    Cancel {
        session: SessionId,
    },
    Screenshot {
        target: Target,
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
    /// Runs one tool directly. Answered with `ToolResult` carrying the same `call`.
    CallTool {
        call: u64,
        name: String,
        #[serde(default)]
        args: Value,
    },
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
            },
            json!({"type":"prompt","text":"hi"}),
        );
        check(
            Prompt {
                session: Some(sid()),
                text: "hi".into(),
            },
            json!({"type":"prompt","session":SID,"text":"hi"}),
        );
        check(
            Cancel { session: sid() },
            json!({"type":"cancel","session":SID}),
        );
        check(
            Screenshot {
                target: Target::Monitor { id: 2 },
            },
            json!({"type":"screenshot","target":{"type":"monitor","id":2}}),
        );
        check(
            Screenshot {
                target: Target::Window { id: 3 },
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
