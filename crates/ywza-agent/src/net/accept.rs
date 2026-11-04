//! Incoming control connections: handshake, permissions, then the message loop.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use iroh::endpoint::Incoming;
use tokio::sync::mpsc;
use ywza_proto::{read_frame, AgentMsg, ClientMsg};

use super::{dispatch::Client, peer, writer::write_loop};
use crate::Agent;

pub async fn accept_loop(agent: Arc<Agent>) {
    while let Some(incoming) = agent.endpoint.accept().await {
        let agent = agent.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_client(agent, incoming).await {
                eprintln!("ywza-agent: client: {e}");
            }
        });
    }
}

async fn serve_client(agent: Arc<Agent>, incoming: Incoming) -> anyhow::Result<()> {
    let mut c = tokio::time::timeout(
        Duration::from_secs(30),
        ywza_proto::accept_control(incoming),
    )
    .await??;
    let peer_id = c.conn.remote_id();
    let hello = tokio::time::timeout(Duration::from_secs(30), read_frame(&mut c.recv)).await??;
    let ClientMsg::Hello { client_name, .. } = hello else {
        c.conn.close(1u32.into(), b"expected hello");
        anyhow::bail!("{peer_id}: first message was not hello");
    };
    // Before the first lookup, so no permission change falls in between.
    let mut changes = agent.config.subscribe();
    let local = peer_id == agent.local_client;
    let Some(mut perms) = peer::permissions(&agent, peer_id) else {
        c.conn.close(2u32.into(), b"not paired");
        anyhow::bail!("{peer_id} ({client_name}) rejected: not paired");
    };

    // Subscribe before Welcome so no event falls in between.
    let events = agent.tx.subscribe();
    let see_sessions = Arc::new(AtomicBool::new(perms.chat || perms.tools));
    let (direct, direct_rx) = mpsc::unbounded_channel();
    direct.send(AgentMsg::Welcome {
        agent_version: env!("CARGO_PKG_VERSION").into(),
        node_id: agent.endpoint.id().to_string(),
        session_list: if perms.chat {
            agent.sessions.list()
        } else {
            Vec::new()
        },
        permissions: (!local).then_some(perms),
    })?;
    let writer = tokio::spawn(write_loop(c.send, events, direct_rx, see_sessions.clone()));
    // Frames are read in their own task: a read cut short by a config change would lose bytes.
    let (frames_tx, mut frames) = mpsc::channel(16);
    let mut recv = c.recv;
    let reader = tokio::spawn(async move {
        loop {
            let r = read_frame::<_, ClientMsg>(&mut recv).await;
            let fatal = matches!(r, Err(ref e) if !matches!(e, ywza_proto::Error::Json(_)));
            if frames_tx.send(r).await.is_err() || fatal {
                return;
            }
        }
    });
    let mut client = Client::new(
        agent.clone(),
        c.conn.clone(),
        client_name.clone(),
        direct.clone(),
    );

    loop {
        let frame = tokio::select! {
            biased;
            r = changes.changed() => {
                if r.is_err() {
                    break;
                }
                let Some(now) = peer::permissions(&agent, peer_id) else {
                    c.conn.close(2u32.into(), b"not paired");
                    break;
                };
                perms = now;
                see_sessions.store(perms.chat || perms.tools, Ordering::Relaxed);
                if !perms.view {
                    client.stop_live();
                }
                continue;
            }
            f = frames.recv() => match f {
                Some(f) => f,
                None => break,
            },
        };
        let msg = match frame {
            Ok(msg) => msg,
            Err(ywza_proto::Error::Json(e)) => {
                let _ = direct.send(AgentMsg::Error {
                    message: format!("bad message: {e}"),
                });
                continue;
            }
            Err(_) => break,
        };
        match peer::denied(&perms, &msg) {
            Some(reply) => {
                let _ = direct.send(reply);
            }
            None => client.handle(msg),
        }
    }
    if let Some(why) = c.conn.close_reason() {
        eprintln!("ywza-agent: {client_name} ({peer_id}) disconnected: {why}");
    }
    client.finish();
    drop(direct);
    reader.abort();
    writer.abort();
    Ok(())
}
