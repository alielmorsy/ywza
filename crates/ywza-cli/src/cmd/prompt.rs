use std::io::IsTerminal;

use anyhow::{bail, Result};
use ywza_proto::{read_frame, write_frame, AgentMsg, ClientMsg, Control, SessionEvent, SessionId};

use crate::render::format_event;

pub async fn run(
    c: &mut Control,
    text: String,
    session: Option<SessionId>,
    verbose: bool,
) -> Result<()> {
    let color = std::io::stdout().is_terminal();
    let msg = ClientMsg::Prompt {
        session,
        text: text.clone(),
    };
    write_frame(&mut c.send, &msg).await?;
    // Every client sees every session; a new session's id is learned from its first event.
    let mut ours = session;
    loop {
        match read_frame(&mut c.recv).await? {
            AgentMsg::Event { session, event } => {
                let ours_first = match &event {
                    SessionEvent::User { text: t } => *t == text,
                    _ => true,
                };
                if ours.is_none() && ours_first {
                    ours = Some(session);
                }
                if ours != Some(session) {
                    continue;
                }
                println!("{}", format_event(&event, verbose, color));
                match event {
                    SessionEvent::Done { .. } => return Ok(()),
                    SessionEvent::Failed { .. } => bail!("session failed"),
                    _ => {}
                }
            }
            AgentMsg::Error { message } => bail!("agent: {message}"),
            _ => {}
        }
    }
}
