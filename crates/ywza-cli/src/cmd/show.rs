use std::io::IsTerminal;

use anyhow::{bail, Result};
use ywza_proto::{AgentMsg, ClientMsg, Control, SessionId};

use crate::{connect::ask, render::format_event};

pub async fn run(c: &mut Control, session: SessionId, verbose: bool) -> Result<()> {
    let color = std::io::stdout().is_terminal();
    match ask(c, ClientMsg::GetSession { session }).await? {
        AgentMsg::Session { events, .. } => {
            for e in events {
                println!("{}", format_event(&e, verbose, color));
            }
            Ok(())
        }
        m => bail!("unexpected reply {m:?}"),
    }
}
