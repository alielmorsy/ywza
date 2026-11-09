//! `ywza-agent pair` talks to the running agent through the config dir: it drops
//! `pairing-request`; the agent answers in `pairing.code` (`<code> <seconds>` or `error: ...`).

use std::{sync::Arc, time::Duration};

use ywza_proto::AgentMsg;

use super::pair;
use crate::{atomic::write_atomic, Agent};

pub async fn watch_pairing_requests(agent: Arc<Agent>) {
    let req = agent.dir.join("pairing-request");
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if std::fs::remove_file(&req).is_ok() {
            tracing::debug!("pairing requested through the config dir");
            let answer = match pair::create(&agent).await {
                AgentMsg::PairingCode {
                    code,
                    expires_in_secs,
                } => format!("{code} {expires_in_secs}"),
                AgentMsg::Error { message } => format!("error: {message}"),
                _ => continue,
            };
            if let Err(e) = write_atomic(&agent.dir.join("pairing.code"), &answer) {
                tracing::error!(error = %e, "writing pairing.code failed");
            }
        }
    }
}
