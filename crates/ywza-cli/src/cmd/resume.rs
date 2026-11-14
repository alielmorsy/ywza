use anyhow::Result;
use ywza_proto::{client::ask, ClientMsg, Control, SessionId};

/// Hands a session that asked for help back to its AI. The agent answers only with an error;
/// any other reply (the `resumed` event) means it took.
pub async fn run(c: &mut Control, session: SessionId, note: Option<String>) -> Result<()> {
    let note = note.unwrap_or_default();
    ask(c, ClientMsg::Resume { session, note }).await?;
    println!("resumed");
    Ok(())
}
