//! Who a connection is and what it may do: the permission gate.

use iroh::EndpointId;
use ywza_proto::{AgentMsg, ClientMsg, Permissions, SessionEvent};

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

/// Which broadcasts a client gets: sessions and approvals need chat or tools, config needs settings.
pub fn visible(msg: &AgentMsg, perms: &Permissions) -> bool {
    match msg {
        AgentMsg::Event { .. }
        | AgentMsg::ApprovalRequest { .. }
        | AgentMsg::ApprovalResolved { .. } => perms.chat || perms.tools,
        AgentMsg::Config { .. } => perms.settings,
        _ => true,
    }
}

/// Drops pictures shown to the user (`show_screenshot`) for a client that may not view the screen.
pub fn redact(mut msg: AgentMsg, perms: &Permissions) -> AgentMsg {
    if !perms.view {
        let strip = |e: &mut SessionEvent| {
            if let SessionEvent::ToolResult { image, .. } = e {
                *image = None;
            }
        };
        match &mut msg {
            AgentMsg::Event { event, .. } => strip(event),
            AgentMsg::Session { events, .. } => events.iter_mut().for_each(strip),
            _ => {}
        }
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use ywza_proto::SessionId;

    #[test]
    fn shown_images_need_view() {
        let shown = || AgentMsg::Event {
            session: SessionId::new(),
            event: SessionEvent::ToolResult {
                id: "c".into(),
                ok: true,
                text: String::new(),
                has_image: false,
                image: Some("png".into()),
            },
        };
        let image = |m: AgentMsg| match m {
            AgentMsg::Event {
                event: SessionEvent::ToolResult { image, .. },
                ..
            } => image,
            _ => unreachable!(),
        };
        assert!(image(redact(shown(), &Permissions::all())).is_some());
        let chat_only = Permissions {
            view: false,
            ..Permissions::all()
        };
        assert!(image(redact(shown(), &chat_only)).is_none());
    }

    const NONE: Permissions = Permissions {
        view: false,
        chat: false,
        control: false,
        tools: false,
        settings: false,
        drive: false,
    };

    #[test]
    fn broadcasts_follow_permissions() {
        let all = Permissions::all();
        let event = AgentMsg::Event {
            session: SessionId::new(),
            event: SessionEvent::Done {
                summary: String::new(),
            },
        };
        assert!(visible(&event, &all));
        assert!(!visible(&event, &NONE));
        let err = AgentMsg::Error {
            message: String::new(),
        };
        assert!(visible(&err, &NONE));
        let resolved = AgentMsg::ApprovalResolved {
            request: 1,
            allowed: true,
        };
        assert!(visible(&resolved, &all) && !visible(&resolved, &NONE));
        let config = AgentMsg::Config {
            config: Default::default(),
        };
        let chat_only = Permissions { chat: true, ..NONE };
        assert!(visible(&config, &all) && !visible(&config, &chat_only));
    }

    #[test]
    fn gate_names_the_missing_permission() {
        let none = NONE;
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
