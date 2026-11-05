#![allow(dead_code)]
//! In-process agent + mock OpenAI server + real iroh clients over loopback (offline).

use std::{sync::Arc, time::Duration};

use iroh::{endpoint::presets, Endpoint, RelayMode, SecretKey};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use ywza_agent::Agent;
use ywza_proto::{
    read_frame, write_frame, AgentMsg, ClientMsg, Config, Control, SessionEvent, Ticket, ALPN,
};

/// Stateless mock: tool call first, final text once a tool result is in the history.
/// The tool call depends on the prompt: "batch-wait", "batch-no-list" or (else) run_command.
pub async fn mock_llm() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let body_start = loop {
                    let n = s.read(&mut chunk).await.unwrap();
                    assert!(n > 0, "client closed early");
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buf[..body_start]).to_lowercase();
                let len: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .map(|v| v.trim().parse().unwrap())
                    .unwrap_or(0);
                while buf.len() < body_start + len {
                    let n = s.read(&mut chunk).await.unwrap();
                    buf.extend_from_slice(&chunk[..n]);
                }
                let req: Value = serde_json::from_slice(&buf[body_start..]).unwrap();
                let last = req["messages"].as_array().unwrap().last().unwrap();
                let prompt = req["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| m["role"] == "user")
                    .map_or("", |m| m["content"].as_str().unwrap_or_default());
                let message = if last["role"] == "tool" {
                    json!({"role":"assistant","content":"all done"})
                } else {
                    let (name, args) = match prompt {
                        "batch-wait" => (
                            "batch",
                            json!({"window":"w1","actions":[
                                {"tool":"wait","ms":5},{"tool":"wait","ms":10}]}),
                        ),
                        // w1 has no element list, so the type_text fails before touching anything.
                        "batch-no-list" => (
                            "batch",
                            json!({"window":"w1","actions":[
                                {"tool":"wait","ms":5},
                                {"tool":"type_text","element":1,"text":"hi","submit":true}]}),
                        ),
                        "bad-args" => ("click", json!({"window":"w1"})),
                        _ => ("run_command", json!({"command":"echo hi"})),
                    };
                    json!({"role":"assistant","content":null,"tool_calls":[{
                        "id":"call_1","type":"function",
                        "function":{"name":name,"arguments":args.to_string()}}]})
                };
                let body =
                    json!({"choices":[{"index":0,"message":message,"finish_reason":"stop"}]})
                        .to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                s.write_all(resp.as_bytes()).await.unwrap();
                s.shutdown().await.ok();
            });
        }
    });
    format!("http://{addr}/v1")
}

/// A loopback iroh client with no relay and no address lookup.
pub async fn endpoint(key: Option<SecretKey>) -> Endpoint {
    let mut b = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .alpns(vec![ALPN.to_vec()]);
    if let Some(k) = key {
        b = b.secret_key(k);
    }
    b.bind().await.unwrap()
}

/// An offline agent in a fresh dir. `llm` points the config at the mock server.
pub async fn agent(llm: bool) -> (tempfile::TempDir, Arc<Agent>) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    if llm {
        cfg.llm.base_url = mock_llm().await;
        cfg.llm.model = "mock".into();
    }
    std::fs::write(
        dir.path().join("config.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();
    let agent = Agent::start(dir.path().to_path_buf(), true).await.unwrap();
    (dir, agent)
}

pub fn ticket(dir: &std::path::Path) -> Ticket {
    std::fs::read_to_string(dir.join("agent.ticket"))
        .unwrap()
        .parse()
        .unwrap()
}

pub async fn hello(client: &Endpoint, ticket: &Ticket, name: &str) -> Control {
    let mut c = ywza_proto::connect(client, ticket).await.unwrap();
    write_frame(
        &mut c.send,
        &ClientMsg::Hello {
            client_name: name.into(),
            token: None,
        },
    )
    .await
    .unwrap();
    c
}

/// The local client of an agent, welcomed. Returns the client endpoint too: dropping it
/// kills the connection.
pub async fn start(llm: bool) -> (tempfile::TempDir, Endpoint, Control) {
    let (dir, _agent) = agent(llm).await;
    let key = ywza_proto::load_or_create_secret_key(&dir.path().join("local-client.key")).unwrap();
    let client = endpoint(Some(key)).await;
    let mut c = hello(&client, &ticket(dir.path()), "test").await;
    let welcome: AgentMsg = read_frame(&mut c.recv).await.unwrap();
    assert!(matches!(welcome, AgentMsg::Welcome { .. }), "{welcome:?}");
    (dir, client, c)
}

/// Sends a prompt and collects session events until done/failed, plus the other messages.
pub async fn run_prompt(c: &mut Control, text: &str) -> (Vec<SessionEvent>, Vec<AgentMsg>) {
    write_frame(
        &mut c.send,
        &ClientMsg::Prompt {
            session: None,
            text: text.into(),
        },
    )
    .await
    .unwrap();
    let mut events = Vec::new();
    let mut others = Vec::new();
    loop {
        let msg: AgentMsg = tokio::time::timeout(Duration::from_secs(30), read_frame(&mut c.recv))
            .await
            .expect("timed out waiting for the agent")
            .unwrap();
        match msg {
            AgentMsg::Event { event, .. } => {
                let end = matches!(
                    event,
                    SessionEvent::Done { .. } | SessionEvent::Failed { .. }
                );
                events.push(event);
                if end {
                    return (events, others);
                }
            }
            other => others.push(other),
        }
    }
}
