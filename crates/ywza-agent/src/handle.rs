use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use ywza_proto::{AgentMsg, ClientMsg, SessionEvent, SessionId, ToolInfo};

use crate::{llm, Agent};

impl Agent {
    /// Handles one client message; the return value goes back to that client only.
    pub fn handle(self: &Arc<Self>, msg: ClientMsg) -> Option<AgentMsg> {
        let err = |m: &str| Some(AgentMsg::Error { message: m.into() });
        match msg {
            ClientMsg::Hello { .. } => None,
            ClientMsg::Prompt { session, text } => {
                let sid = session.unwrap_or_else(|| self.sessions.create(&text));
                match self.sessions.begin(sid) {
                    Ok((cancel, history)) => {
                        tokio::spawn(self.clone().run_prompt(sid, text, cancel, history));
                        None
                    }
                    Err(e) => err(e),
                }
            }
            ClientMsg::Cancel { session } => {
                (!self.sessions.cancel(session)).then(|| AgentMsg::Error {
                    message: "session is not running".into(),
                })
            }
            ClientMsg::GetConfig => Some(AgentMsg::Config {
                config: self.config.get().masked(),
            }),
            ClientMsg::SetConfig { config } => match self.config.set(config) {
                Ok(()) => Some(AgentMsg::Config {
                    config: self.config.get().masked(),
                }),
                Err(e) => err(&format!("saving config: {e}")),
            },
            ClientMsg::ListSessions => Some(AgentMsg::Sessions {
                list: self.sessions.list(),
            }),
            ClientMsg::ListTools => Some(AgentMsg::Tools {
                list: self
                    .tools
                    .specs()
                    .into_iter()
                    .map(|s| ToolInfo {
                        name: s.name.into(),
                        description: s.description,
                        parameters: s.parameters,
                    })
                    .collect(),
            }),
            ClientMsg::GetSession { session } => match self.sessions.get(session) {
                Some(events) => Some(AgentMsg::Session { session, events }),
                None => err("unknown session"),
            },
            // Per-connection or async (peer/network, capture); answered by the connection.
            ClientMsg::CreatePairing
            | ClientMsg::Screenshot { .. }
            | ClientMsg::LiveStart { .. }
            | ClientMsg::LiveStop
            | ClientMsg::Input { .. }
            | ClientMsg::CallTool { .. } => err("not handled here"),
        }
    }

    async fn run_prompt(
        self: Arc<Self>,
        sid: SessionId,
        text: String,
        cancel: CancellationToken,
        mut history: Vec<llm::Msg>,
    ) {
        self.emit(sid, SessionEvent::User { text: text.clone() });
        history.push(llm::Msg::new(
            serde_json::json!({"role":"user","content":text}),
        ));
        let result = llm::run(&self, sid, &mut history, &cancel).await;
        // Free the session before announcing the end, so an immediate follow-up prompt is accepted.
        self.sessions.finish(sid, history);
        self.emit(
            sid,
            match result {
                Ok(summary) => SessionEvent::Done { summary },
                Err(error) => SessionEvent::Failed { error },
            },
        );
    }
}
