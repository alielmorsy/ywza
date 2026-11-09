use anyhow::{bail, Result};
use ywza_proto::{AgentMsg, ClientMsg, Control};

use crate::connect::ask;

pub async fn run(c: &mut Control) -> Result<()> {
    match ask(c, ClientMsg::ListSessions).await? {
        AgentMsg::Sessions { list } => {
            if list.is_empty() {
                println!("no sessions");
            }
            for s in list {
                println!("{}  {:?}  {}", s.id.0, s.status, s.title);
            }
            Ok(())
        }
        m => bail!("unexpected reply {m:?}"),
    }
}
