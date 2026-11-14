//! In-process agent + mock OpenAI server + real iroh client over loopback (offline).

mod common;

use std::time::Duration;

use common::{agent, collect, endpoint, hello, run_prompt, send_prompt, start, ticket};
use serde_json::{json, Value};
use ywza_proto::{
    read_frame, write_frame, AgentMsg, ApprovalMode, ClientMsg, Permissions, SessionEvent,
};

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
                if text.contains("missing field `text`"))),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(SessionEvent::Done { .. })),
        "{events:?}"
    );
}

/// Answers every approval request with `allow`; returns what was asked and the session events.
async fn answer_asks(
    c: &mut ywza_proto::Control,
    allow: bool,
    remember: bool,
) -> (Vec<(String, String)>, Vec<SessionEvent>) {
    let (mut asked, mut events) = (Vec::new(), Vec::new());
    loop {
        let msg: AgentMsg = tokio::time::timeout(Duration::from_secs(30), read_frame(&mut c.recv))
            .await
            .expect("timed out waiting for the agent")
            .unwrap();
        match msg {
            AgentMsg::ApprovalRequest {
                request,
                tool,
                summary,
                ..
            } => {
                asked.push((tool, summary));
                let answer = ClientMsg::Approve {
                    request,
                    allow,
                    remember,
                };
                write_frame(&mut c.send, &answer).await.unwrap();
            }
            AgentMsg::Event { event, .. } => {
                let end = matches!(
                    event,
                    SessionEvent::Done { .. } | SessionEvent::Failed { .. }
                );
                events.push(event);
                if end {
                    return (asked, events);
                }
            }
            _ => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn run_command_asks_and_a_no_reaches_the_model_as_a_denied_result() {
    let (_dir, _client, mut c) = start(true).await;
    send_prompt(&mut c, "say hi", Some(ApprovalMode::Rules)).await;
    let (asked, events) = answer_asks(&mut c, false, false).await;
    assert_eq!(asked, [("run_command".to_string(), "echo hi".to_string())]);
    assert!(
        events.iter().any(|e| matches!(e,
            SessionEvent::ToolResult { ok: false, text, .. } if text == "denied by the user")),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(SessionEvent::Done { .. })),
        "{events:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_yes_with_remember_runs_the_call_and_saves_an_allow_rule() {
    let (dir, _client, mut c) = start(true).await;
    send_prompt(&mut c, "say hi", Some(ApprovalMode::Rules)).await;
    let (asked, events) = answer_asks(&mut c, true, true).await;
    assert_eq!(asked.len(), 1);
    assert!(
        events.iter().any(|e| matches!(e,
            SessionEvent::ToolResult { ok: true, text, .. } if text.contains("hi"))),
        "{events:?}"
    );
    let saved = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
    let cfg: ywza_proto::Config = toml::from_str(&saved).unwrap();
    let first = &cfg.approval.rules[0];
    assert_eq!(
        (first.tool.as_str(), first.action),
        ("run_command", ywza_proto::Action::Allow)
    );

    // Allowed from now on: a second run does not ask.
    send_prompt(&mut c, "say hi", Some(ApprovalMode::Rules)).await;
    let (asked, _) = answer_asks(&mut c, false, false).await;
    assert!(asked.is_empty(), "{asked:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_never_asks() {
    let (_dir, _client, mut c) = start(true).await;
    send_prompt(&mut c, "say hi", Some(ApprovalMode::Auto)).await;
    let (events, others) = collect(&mut c).await;
    assert_full_run(&events);
    assert!(
        !others
            .iter()
            .any(|m| matches!(m, AgentMsg::ApprovalRequest { .. })),
        "{others:?}"
    );
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
    assert_eq!(list.len(), 23);
    assert!(list
        .iter()
        .any(|t| t.name == "wait" && t.parameters["type"] == "object"));

    let call = |call, name: &str, args| ClientMsg::CallTool {
        call,
        name: name.into(),
        args,
        space: None,
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
    let AgentMsg::ToolResult {
        call: 10, ok, text, ..
    } = reply(&mut s).await
    else {
        panic!("expected a tool result");
    };
    assert!(!ok && text.contains("`ms`"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn local_tcp_port_creates_pairing_codes() {
    let (dir, _client, _c) = start(false).await;
    let mut s = tcp_session(dir.path()).await;
    write_frame(&mut s, &ClientMsg::CreatePairing)
        .await
        .unwrap();
    assert!(matches!(
        reply(&mut s).await,
        AgentMsg::PairingCode {
            expires_in_secs: 600,
            ..
        }
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_tool_calls_run_without_asking() {
    let (dir, _client, _c) = start(false).await;
    let mut s = tcp_session(dir.path()).await;
    let msg = ClientMsg::CallTool {
        call: 1,
        name: "run_command".into(),
        args: json!({"command":"echo hi"}),
        space: None,
    };
    write_frame(&mut s, &msg).await.unwrap();
    assert!(matches!(
        reply(&mut s).await,
        AgentMsg::ToolResult {
            call: 1,
            ok: true,
            ..
        }
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn shown_screenshot_reaches_the_user_but_not_the_model() {
    // shortcut: captures whatever real window is on top, which can hang or vanish mid-test (seen
    // once, 90 s); give the test its own window if it flakes.
    let Some(w) = ywza_desktop::list_windows()
        .ok()
        .and_then(|l| l.into_iter().find(|w| !w.minimized && !w.hidden))
    else {
        return eprintln!("no desktop window to capture; skipped");
    };
    let (_dir, _client, mut c) = start(true).await;
    let (events, _) = run_prompt(&mut c, &format!("show w{}", w.id)).await;
    assert!(
        events.iter().any(|e| matches!(e,
            SessionEvent::ToolResult { ok: true, has_image: false, image: Some(png), .. } if !png.is_empty())),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(SessionEvent::Done { summary }) if summary == "all done"),
        "the model must not get the picture: {events:?}"
    );
}
