//! Code pairing against a real agent over loopback: the rendezvous endpoint, one attempt only.

mod common;

use std::time::Duration;

use common::{agent, endpoint, hello, ticket};
use iroh::Endpoint;
use ywza_agent::net::pair;
use ywza_proto::{client_side, read_frame, AgentMsg, Error, PairInfo, Permissions, PAIR_ALPN};

/// Dials the rendezvous endpoint and runs the client side with `code`.
async fn try_pair(
    client: &Endpoint,
    addr: &iroh::EndpointAddr,
    code: &str,
) -> Result<PairInfo, Error> {
    let conn = client
        .connect(addr.clone(), PAIR_ALPN)
        .await
        .map_err(|e| Error::Pair(e.to_string()))?;
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| Error::Pair(e.to_string()))?;
    let me = PairInfo {
        node_id: client.id(),
        name: "phone".into(),
        addr: None,
        relay: None,
    };
    let r = client_side(&mut send, &mut recv, code, me).await;
    conn.close(0u32.into(), b"done");
    r
}

/// The agent trusts the peer just after the handshake returns on its side.
async fn wait_for_peers(agent: &ywza_agent::Agent) -> Vec<ywza_proto::Peer> {
    for _ in 0..40 {
        let peers = agent.config.get().peers;
        if !peers.is_empty() {
            return peers;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Vec::new()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_right_code_pairs_and_the_peer_reconnects_without_one() {
    let (dir, agent) = agent(false).await;
    let started = pair::begin(&agent).await.unwrap();
    assert_eq!(started.code.len(), 6);
    let phone = endpoint(None).await;

    let info = try_pair(&phone, &started.addr, &started.code)
        .await
        .unwrap();
    assert_eq!(info.node_id, agent.endpoint.id());

    let peers = wait_for_peers(&agent).await;
    assert_eq!(peers.len(), 1);
    assert_eq!(
        (peers[0].id.as_str(), peers[0].name.as_str(), peers[0].perms),
        (
            phone.id().to_string().as_str(),
            "phone",
            Permissions::default()
        )
    );

    let mut c = hello(&phone, &ticket(dir.path()), "phone").await;
    let welcome: AgentMsg = read_frame(&mut c.recv).await.unwrap();
    assert!(matches!(
        welcome,
        AgentMsg::Welcome { permissions: Some(p), .. } if p == Permissions::default()
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn one_wrong_code_burns_it() {
    let (_dir, agent) = agent(false).await;
    let started = pair::begin(&agent).await.unwrap();
    let wrong = if started.code == "000000" {
        "000001"
    } else {
        "000000"
    };
    let phone = endpoint(None).await;

    let err = try_pair(&phone, &started.addr, wrong).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::BadCode | Error::Pair(_) | Error::Io(_) | Error::Transport(_)
        ),
        "{err:?}"
    );
    assert!(agent.config.get().peers.is_empty());

    // The right code now fails too, and the agent says why.
    let conn = phone
        .connect(started.addr.clone(), PAIR_ALPN)
        .await
        .expect("the endpoint still answers to refuse");
    let reason = tokio::time::timeout(Duration::from_secs(10), conn.closed())
        .await
        .expect("refusal not sent");
    assert!(
        reason.to_string().contains("code expired, run pair again"),
        "{reason}"
    );
    assert!(agent.config.get().peers.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_code_supersedes_an_unused_one() {
    let (_dir, agent) = agent(false).await;
    let _first = pair::begin(&agent).await.unwrap();
    let second = pair::begin(&agent).await.unwrap();
    let phone = endpoint(None).await;
    let ok = try_pair(&phone, &second.addr, &second.code).await;
    assert!(ok.is_ok(), "{ok:?}");
}
