//! Tool calls from external clients (the local TCP port, paired devices): each client gets
//! one session, so its calls show up in the apps like the agent's own.

use serde_json::Value;
use ywza_proto::{AgentMsg, SessionEvent, SessionId};

use crate::Agent;

pub fn session(agent: &Agent, client_name: &str) -> SessionId {
    let sid = agent.sessions.create(&format!("MCP: {client_name}"));
    tracing::info!(session = %sid.0, client = client_name, "tool session started");
    agent.emit(
        sid,
        SessionEvent::User {
            text: format!("{client_name} connected and is driving tools directly"),
        },
    );
    sid
}

pub fn end_session(agent: &Agent, sid: SessionId) {
    tracing::info!(session = %sid.0, "tool session ended");
    agent.emit(
        sid,
        SessionEvent::Done {
            summary: "client disconnected".into(),
        },
    );
}

/// Runs a tool for an external client, logging it in `sid` like the agent's own calls. No approval step.
pub async fn call_tool(
    agent: &Agent,
    sid: SessionId,
    call: u64,
    name: String,
    args: Value,
) -> AgentMsg {
    let id = format!("mcp_{call}");
    agent.emit_tool_call(sid, &id, &name, &args, None);
    let out = agent.run_tool(&name, &args).await;
    agent.emit_tool_result(sid, &id, &out);
    AgentMsg::ToolResult {
        call,
        ok: out.ok,
        text: out.text,
        png_base64: out.image_png.map(|p| data_encoding::BASE64.encode(&p)),
    }
}
