//! Local TCP port for scripts and MCP servers on this machine (e.g. `python/ywza_mcp`).
//!
//! Same frames as the iroh control stream (u32 LE length + JSON) on 127.0.0.1. The agent writes
//! `<config_dir>/local-tcp.json` (`{"port", "token"}`) at start; the first frame must be
//! `hello` with `token` = that token. Only the local user can read the config dir, so the
//! token plays the role `local-client.key` plays for iroh clients.

use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use data_encoding::BASE32_NOPAD;
use serde_json::json;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use ywza_proto::{read_frame, AgentMsg, ClientMsg};

use crate::{atomic::write_atomic, mcp, net, Agent};

pub const FILE: &str = "local-tcp.json";

/// Binds 127.0.0.1 on a free port and writes a fresh token next to it.
pub async fn bind(dir: &Path) -> anyhow::Result<(TcpListener, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS random source");
    let token = BASE32_NOPAD.encode(&bytes);
    let info = json!({"port": listener.local_addr()?.port(), "token": token});
    write_atomic(&dir.join(FILE), &info.to_string())?;
    Ok((listener, token))
}

pub async fn accept_loop(agent: Arc<Agent>, (listener, token): (TcpListener, String)) {
    let token = Arc::new(token);
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let (agent, token) = (agent.clone(), token.clone());
        tokio::spawn(async move {
            if let Err(e) = serve(agent, stream, &token).await {
                tracing::warn!(error = %e, "local client failed");
            }
        });
    }
}

async fn serve(agent: Arc<Agent>, stream: TcpStream, token: &str) -> anyhow::Result<()> {
    let _ = stream.set_nodelay(true);
    let (mut recv, send) = stream.into_split();
    let hello = tokio::time::timeout(Duration::from_secs(10), read_frame(&mut recv)).await??;
    let ClientMsg::Hello {
        client_name,
        token: Some(given),
    } = hello
    else {
        anyhow::bail!("first message was not hello with a token");
    };
    if given != token {
        anyhow::bail!("{client_name}: wrong token");
    }

    tracing::info!(client = client_name, "local client connected");
    let events = agent.tx.subscribe();
    let (direct, direct_rx) = mpsc::unbounded_channel();
    direct.send(AgentMsg::Welcome {
        agent_version: env!("CARGO_PKG_VERSION").into(),
        node_id: agent.endpoint.id().to_string(),
        session_list: agent.sessions.list(),
        os: Some(crate::llm::os_description().into()),
        permissions: None,
    })?;
    for request in agent.approvals.requests(None) {
        direct.send(request)?;
    }
    // The local user: every permission.
    let see_sessions = Arc::new(AtomicBool::new(true));
    let writer = tokio::spawn(net::write_loop(send, events, direct_rx, see_sessions));
    let mut session = None;

    loop {
        let frame = read_frame::<_, ClientMsg>(&mut recv).await;
        if let Ok(msg) = &frame {
            tracing::debug!(
                client = client_name,
                kind = msg.kind(),
                "local client message"
            );
        }
        let reply = match frame {
            Ok(ClientMsg::CallTool { call, name, args }) => {
                let sid = *session.get_or_insert_with(|| mcp::session(&agent, &client_name));
                let (agent, direct) = (agent.clone(), direct.clone());
                tokio::spawn(async move {
                    let _ = direct.send(mcp::call_tool(&agent, sid, call, name, args).await);
                });
                None
            }
            Ok(ClientMsg::Approve {
                request,
                allow,
                remember,
            }) => agent.approve(request, allow, remember),
            Ok(ClientMsg::Screenshot { target }) => Some(agent.screenshot(target).await),
            Ok(ClientMsg::CreatePairing) => {
                let (agent, direct) = (agent.clone(), direct.clone());
                tokio::spawn(async move {
                    let _ = direct.send(net::pair::create(&agent).await);
                });
                None
            }
            Ok(
                m
                @ (ClientMsg::ListMonitors | ClientMsg::ListModels | ClientMsg::WindowInfo { .. }),
            ) => {
                let (agent, direct) = (agent.clone(), direct.clone());
                tokio::spawn(async move {
                    let _ = direct.send(agent.inspect(m, true).await);
                });
                None
            }
            // Need the iroh connection (live view, file copy).
            Ok(
                ClientMsg::LiveStart { .. }
                | ClientMsg::LiveStop
                | ClientMsg::Input { .. }
                | ClientMsg::Pull { .. }
                | ClientMsg::Push { .. }
                | ClientMsg::PushEnd { .. },
            ) => Some(AgentMsg::Error {
                message: "not available on the local TCP port".into(),
            }),
            Ok(msg) => {
                for r in agent.handle(msg, true) {
                    let _ = direct.send(r);
                }
                None
            }
            Err(ywza_proto::Error::Json(e)) => Some(AgentMsg::Error {
                message: format!("bad message: {e}"),
            }),
            Err(_) => break,
        };
        if let Some(r) = reply {
            let _ = direct.send(r);
        }
    }
    tracing::info!(client = client_name, "local client disconnected");
    if let Some(sid) = session {
        mcp::end_session(&agent, sid);
    }
    drop(direct);
    writer.abort();
    Ok(())
}
