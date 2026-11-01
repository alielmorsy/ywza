use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Config, Permissions, SessionEvent, SessionId, SessionSummary};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments.
    pub parameters: Value,
}

/// Agent to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMsg {
    Welcome {
        agent_version: String,
        node_id: String,
        session_list: Vec<SessionSummary>,
        /// What this client may do; absent means everything (local app, local TCP clients).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permissions: Option<Permissions>,
    },
    Event {
        session: SessionId,
        event: SessionEvent,
    },
    Image {
        png_base64: String,
        width: u32,
        height: u32,
    },
    /// Always [`Config::masked`].
    Config {
        config: Config,
    },
    Sessions {
        list: Vec<SessionSummary>,
    },
    Session {
        session: SessionId,
        events: Vec<SessionEvent>,
    },
    /// Answer to `CreatePairing`: a one-time 6-digit code and its lifetime.
    PairingCode {
        code: String,
        expires_in_secs: u64,
    },
    Tools {
        list: Vec<ToolInfo>,
    },
    /// Live view with `stream.cursor` off: the remote cursor's shape, sent when it changes, so
    /// the client can show it on its own pointer. `png_base64` None: the cursor is hidden.
    Cursor {
        png_base64: Option<String>,
        hot_x: u32,
        hot_y: u32,
    },
    ToolResult {
        call: u64,
        ok: bool,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        png_base64: Option<String>,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::testutil::{check, sid, SID};
    use crate::SessionStatus;
    use serde_json::json;

    fn cfg_json() -> Value {
        serde_json::to_value(Config::default()).unwrap()
    }

    #[test]
    fn agent_msgs() {
        use AgentMsg::*;
        let summary = SessionSummary {
            id: sid(),
            title: "t".into(),
            status: SessionStatus::Running,
            updated_ms: 5,
        };
        let summary_json = json!({"id":SID,"title":"t","status":"running","updated_ms":5});
        check(
            Welcome {
                agent_version: "0.1".into(),
                node_id: "n".into(),
                session_list: vec![summary.clone()],
                permissions: None,
            },
            json!({"type":"welcome","agent_version":"0.1","node_id":"n","session_list":[summary_json]}),
        );
        check(
            Welcome {
                agent_version: "0.1".into(),
                node_id: "n".into(),
                session_list: vec![],
                permissions: Some(Permissions::default()),
            },
            json!({"type":"welcome","agent_version":"0.1","node_id":"n","session_list":[],
                   "permissions":{"view":true,"chat":true,"control":true,"tools":true,
                                  "settings":false}}),
        );
        check(
            Event {
                session: sid(),
                event: SessionEvent::Done {
                    summary: "s".into(),
                },
            },
            json!({"type":"event","session":SID,"event":{"type":"done","summary":"s"}}),
        );
        check(
            Image {
                png_base64: "AA==".into(),
                width: 2,
                height: 3,
            },
            json!({"type":"image","png_base64":"AA==","width":2,"height":3}),
        );
        check(
            Config {
                config: crate::Config::default(),
            },
            json!({"type":"config","config":cfg_json()}),
        );
        check(
            Sessions {
                list: vec![summary],
            },
            json!({"type":"sessions","list":[summary_json]}),
        );
        check(
            Session {
                session: sid(),
                events: vec![SessionEvent::User { text: "u".into() }],
            },
            json!({"type":"session","session":SID,"events":[{"type":"user","text":"u"}]}),
        );
        check(
            PairingCode {
                code: "123456".into(),
                expires_in_secs: 600,
            },
            json!({"type":"pairing_code","code":"123456","expires_in_secs":600}),
        );
        check(
            Tools {
                list: vec![ToolInfo {
                    name: "wait".into(),
                    description: "d".into(),
                    parameters: json!({"type":"object"}),
                }],
            },
            json!({"type":"tools","list":[{"name":"wait","description":"d","parameters":{"type":"object"}}]}),
        );
        check(
            ToolResult {
                call: 4,
                ok: true,
                text: "t".into(),
                png_base64: None,
            },
            json!({"type":"tool_result","call":4,"ok":true,"text":"t"}),
        );
        check(
            Error {
                message: "m".into(),
            },
            json!({"type":"error","message":"m"}),
        );
    }
}
