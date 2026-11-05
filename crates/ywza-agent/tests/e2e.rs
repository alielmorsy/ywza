//! In-process agent + mock OpenAI server + real iroh client over loopback (offline).

mod common;

use std::time::Duration;

use common::{agent, endpoint, hello, run_prompt, start, ticket};
use serde_json::{json, Value};
use ywza_proto::{read_frame, write_frame, AgentMsg, ClientMsg, Permissions, SessionEvent};

fn assert_full_run(events: &[SessionEvent]) {
    let has = |f: &dyn Fn(&SessionEvent) -> bool| events.iter().any(f);
    assert!(
        has(&|e| matches!(e, SessionEvent::ToolCall { name, .. } if name == "run_command")),
        "{events:?}"
    );
    assert!(
        has(
            &|e| matches!(e, SessionEvent::ToolResult { ok: true, text, .. } if text.contains("hi"))
        ),
        "{events:?}"
    );
    assert!(
        has(&|e| matches!(e, SessionEvent::Assistant { text } if text == "all done")),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(SessionEvent::Done { summary }) if summary == "all done"),
        "{events:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn prompt_runs_tool_and_finishes() {
    let (dir, _client, mut c) = start(true).await;
    let (events, _) = run_prompt(&mut c, "say hi").await;
    assert_full_run(&events);
    let logs: Vec<_> = std::fs::read_dir(dir.path().join("logs"))
        .unwrap()
        .collect();
    assert_eq!(logs.len(), 1, "one JSONL activity log");
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_tool_arguments_come_back_as_an_error_result_naming_the_field() {
    let (_dir, _client, mut c) = start(true).await;
    let (events, _) = run_prompt(&mut c, "bad-args").await;
    assert!(
        events.iter().any(|e| matches!(e,
            SessionEvent::ToolResult { ok: false, text, .. }
                if text.contains("missing field `element`"))),
        "{events:?}"
    );
    assert!(matches!(events.last(), Some(SessionEvent::Done { .. })), "{events:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn unpaired_peer_is_rejected() {
    let (dir, _agent) = agent(false).await;
    let stranger = endpoint(None).await;
    let mut c = hello(&stranger, &ticket(dir.path()), "phone").await;
    assert!(read_frame::<_, AgentMsg>(&mut c.recv).await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn trusted_peer_connects_with_its_permissions() {
    let (dir, agent) = agent(false).await;
    let phone = endpoint(None).await;
    agent
        .config
        .trust(phone.id().to_string(), "phone".into())
        .unwrap();
    let mut c = hello(&phone, &ticket(dir.path()), "phone").await;
    let welcome: AgentMsg = read_frame(&mut c.recv).await.unwrap();
    assert!(matches!(
        welcome,
        AgentMsg::Welcome { permissions: Some(p), .. } if p == Permissions::default()
    ));

    // No `settings`: cannot ask for a pairing code.
    write_frame(&mut c.send, &ClientMsg::CreatePairing)
        .await
        .unwrap();
    let reply: AgentMsg = read_frame(&mut c.recv).await.unwrap();
    assert_eq!(
        reply,
        AgentMsg::Error {
            message: "permission denied: settings".into()
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_pairing_answers_with_a_six_digit_code() {
    let (_dir, _client, mut c) = start(false).await;
    write_frame(&mut c.send, &ClientMsg::CreatePairing)
        .await
        .unwrap();
    let AgentMsg::PairingCode {
        code,
        expires_in_secs,
    } = read_frame(&mut c.recv).await.unwrap()
    else {
        panic!("expected a pairing code");
    };
    assert_eq!(code.len(), 6);
    assert!(code.bytes().all(|b| b.is_ascii_digit()));
    assert_eq!(expires_in_secs, 600);
}

#[tokio::test(flavor = "multi_thread")]
async fn batch_of_waits_returns_one_report() {
    let (_dir, _client, mut c) = start(true).await;
    let (events, _) = run_prompt(&mut c, "batch-wait").await;
    assert!(
        events.iter().any(|e| matches!(e,
            SessionEvent::ToolResult { ok: true, text, .. } if text == "1 wait ok\n2 wait ok")),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(SessionEvent::Done { .. })),
        "{events:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn batch_runs_without_asking_and_stops_at_the_first_failure() {
    let (_dir, _client, mut c) = start(true).await;
    let (events, _) = run_prompt(&mut c, "batch-no-list").await;
    assert!(
        events.iter().any(|e| matches!(e,
            SessionEvent::ToolResult { ok: false, text, .. }
                if text.starts_with("1 wait ok\n2 type_text FAILED: no element list for w1"))),
        "{events:?}"
    );
}

async fn tcp_session(dir: &std::path::Path) -> tokio::net::TcpStream {
    let info: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("local-tcp.json")).unwrap())
            .unwrap();
    let mut s = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", info["port"]))
        .await
        .unwrap();
    let hello = ClientMsg::Hello {
        client_name: "py".into(),
        token: Some(info["token"].as_str().unwrap().into()),
    };
    write_frame(&mut s, &hello).await.unwrap();
    let welcome: AgentMsg = read_frame(&mut s).await.unwrap();
    assert!(matches!(welcome, AgentMsg::Welcome { .. }), "{welcome:?}");
    s
}

/// Next non-event message (calls are logged in an "MCP: py" session).
async fn reply(s: &mut tokio::net::TcpStream) -> AgentMsg {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), read_frame(s))
            .await
            .expect("no reply")
            .unwrap()
        {
            AgentMsg::Event { .. } => continue,
            m => return m,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn local_tcp_port_needs_the_token() {
    let (dir, _client, _c) = start(false).await;
    let info: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("local-tcp.json")).unwrap())
            .unwrap();
    let mut bad = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", info["port"]))
        .await
        .unwrap();
    let hello = ClientMsg::Hello {
        client_name: "py".into(),
        token: Some("WRONG".into()),
    };
    write_frame(&mut bad, &hello).await.unwrap();
    assert!(
        read_frame::<_, AgentMsg>(&mut bad).await.is_err(),
        "wrong token is dropped"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn local_tcp_port_lists_and_calls_tools() {
    let (dir, _client, _c) = start(false).await;
    let mut s = tcp_session(dir.path()).await;

    write_frame(&mut s, &ClientMsg::ListTools).await.unwrap();
    let AgentMsg::Tools { list } = reply(&mut s).await else {
        panic!("expected tools")
    };
    assert_eq!(list.len(), 22);
    assert!(list
        .iter()
        .any(|t| t.name == "wait" && t.parameters["type"] == "object"));

    let call = |call, name: &str, args| ClientMsg::CallTool {
        call,
        name: name.into(),
        args,
    };
    write_frame(&mut s, &call(9, "wait", json!({"ms":5})))
        .await
        .unwrap();
    assert_eq!(
        reply(&mut s).await,
        AgentMsg::ToolResult {
            call: 9,
            ok: true,
            text: "waited 5 ms".into(),
            png_base64: None,
        }
    );

    // Bad JSON arguments are an error result that names the field, not a dropped call.
    write_frame(&mut s, &call(10, "wait", json!({"ms":"soon"})))
        .await
        .unwrap();
    let AgentMsg::ToolResult { call: 10, ok, text, .. } = reply(&mut s).await else {
        panic!("expected a tool result");
    };
    assert!(!ok && text.contains("`ms`"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn local_tcp_port_creates_pairing_codes() {
    let (dir, _client, _c) = start(false).await;
    let mut s = tcp_session(dir.path()).await;
    write_frame(&mut s, &ClientMsg::CreatePairing).await.unwrap();
    assert!(matches!(
        reply(&mut s).await,
        AgentMsg::PairingCode { expires_in_secs: 600, .. }
    ));
}
