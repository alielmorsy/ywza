//! Who a connection is and what it may do: the permission gate.

use iroh::EndpointId;
use ywza_proto::{AgentMsg, ClientMsg, Permissions};

use crate::Agent;

/// The local client holds everything; others need a `[[peers]]` entry. None: not paired.
pub fn permissions(agent: &Agent, peer: EndpointId) -> Option<Permissions> {
    if peer == agent.local_client {
        return Some(Permissions::all());
    }
    agent.config.peer(&peer.to_string()).map(|p| p.perms)
}

/// The error reply for a message the peer's permissions do not cover.
pub fn denied(perms: &Permissions, msg: &ClientMsg) -> Option<AgentMsg> {
    perms.missing(msg).map(|need| AgentMsg::Error {
        message: format!("permission denied: {need}"),
    })
}

/// Session events go only to clients allowed to see sessions (chat or tools).
pub fn visible(msg: &AgentMsg, see_sessions: bool) -> bool {
    see_sessions || !matches!(msg, AgentMsg::Event { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ywza_proto::{SessionEvent, SessionId};

    #[test]
    fn events_need_session_access() {
        let event = AgentMsg::Event {
            session: SessionId::new(),
            event: SessionEvent::Done {
                summary: String::new(),
            },
        };
        assert!(visible(&event, true));
        assert!(!visible(&event, false));
        let err = AgentMsg::Error {
            message: String::new(),
        };
        assert!(visible(&err, false));
    }

    #[test]
    fn gate_names_the_missing_permission() {
        let none = Permissions {
            view: false,
            chat: false,
            control: false,
            tools: false,
            settings: false,
        };
        let reply = denied(&none, &ClientMsg::CreatePairing);
        assert_eq!(
            reply,
            Some(AgentMsg::Error {
                message: "permission denied: settings".into()
            })
        );
        assert_eq!(denied(&Permissions::all(), &ClientMsg::CreatePairing), None);
    }
}
