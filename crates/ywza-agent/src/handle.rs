use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use ywza_proto::{AgentMsg, ClientMsg, SessionEvent, SessionId};

use crate::{llm, Agent};

/// How one prompt runs: on which device (None: this machine) and with which overrides.
struct Run {
    device: Option<String>,
    opts: llm::Opts,
}

impl Agent {
    /// Handles one client message; the return value goes back to that client only.
    /// `local`: the client is this machine's own app, which may run sessions on paired devices.
    /// A `GetSession` answer is followed by the session's pending approval requests.
    pub fn handle(self: &Arc<Self>, msg: ClientMsg, local: bool) -> Vec<AgentMsg> {
        let asked = match &msg {
            ClientMsg::GetSession { session } => Some(*session),
            _ => None,
        };
        let mut out: Vec<_> = self.reply(msg, local).into_iter().collect();
        if let (Some(session), Some(AgentMsg::Session { .. })) = (asked, out.first()) {
            out.extend(self.approvals.requests(Some(session)));
        }
        out
    }

    fn reply(self: &Arc<Self>, msg: ClientMsg, local: bool) -> Option<AgentMsg> {
        let err = |m: &str| Some(AgentMsg::Error { message: m.into() });
        match msg {
            ClientMsg::Hello { .. } => None,
            ClientMsg::Prompt {
                session,
                text,
                device,
                model,
                effort,
                approval,
            } => {
                let sid = session.unwrap_or_default();
                let device = match self.sessions.device(sid) {
                    Some(existing) if existing != device => {
                        return err("the session runs on a different device");
                    }
                    Some(existing) => existing,
                    None => {
                        if device.is_some() && !local {
                            return err("only local clients can run a session on a device");
                        }
                        self.sessions.create_with(sid, &text, device.as_deref());
                        device
                    }
                };
                if device.is_some() && !local {
                    return err("only local clients can run a session on a device");
                }
                let (model, effort) = if local { (model, effort) } else { (None, None) };
                if let (true, Some(mode)) = (local, approval) {
                    self.sessions.set_approval(sid, mode);
                }
                match self.sessions.begin(sid) {
                    Ok((cancel, history)) => {
                        let run = Run {
                            device,
                            opts: llm::Opts { model, effort },
                        };
                        tokio::spawn(self.clone().run_prompt(sid, text, cancel, history, run));
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
                Ok(()) => {
                    tracing::debug!("settings changed by a client");
                    Some(AgentMsg::Config {
                        config: self.config.get().masked(),
                    })
                }
                Err(e) => {
                    tracing::error!(error = %e, "saving config failed");
                    err(&format!("saving config: {e}"))
                }
            },
            ClientMsg::ListSessions => Some(AgentMsg::Sessions {
                list: self.sessions.list(),
            }),
            ClientMsg::ListTools => Some(AgentMsg::Tools {
                list: self.tools.infos(),
            }),
            ClientMsg::GetSession { session } => match self.sessions.get(session) {
                Some(events) => Some(AgentMsg::Session { session, events }),
                None => err("unknown session"),
            },
            // Per-connection or async (peer/network, capture); answered by the connection.
            ClientMsg::CreatePairing
            | ClientMsg::ListMonitors
            | ClientMsg::ListModels
            | ClientMsg::WindowInfo { .. }
            | ClientMsg::Screenshot { .. }
            | ClientMsg::LiveStart { .. }
            | ClientMsg::LiveStop
            | ClientMsg::Input { .. }
            | ClientMsg::CallTool { .. }
            | ClientMsg::Approve { .. }
            | ClientMsg::Pull { .. }
            | ClientMsg::Push { .. }
            | ClientMsg::PushEnd { .. } => err("not handled here"),
        }
    }

    async fn run_prompt(
        self: Arc<Self>,
        sid: SessionId,
        text: String,
        cancel: CancellationToken,
        mut history: Vec<llm::Msg>,
        run: Run,
    ) {
        self.emit(sid, SessionEvent::User { text: text.clone() });
        history.push(llm::Msg::new(
            serde_json::json!({"role":"user","content":text}),
        ));
        tracing::info!(session = %sid.0, prompt_chars = text.chars().count(), "session started");
        let result = match &run.device {
            None => {
                llm::run(
                    &self,
                    sid,
                    &mut history,
                    &cancel,
                    &run.opts,
                    &llm::Local(&*self),
                )
                .await
            }
            Some(device) => {
                let opened = tokio::select! {
                    r = self.remotes.open(device) => r.map_err(|e| format!("{e:#}")),
                    _ = cancel.cancelled() => Err("cancelled".to_string()),
                };
                match opened {
                    Ok(remote) => {
                        llm::run(&self, sid, &mut history, &cancel, &run.opts, &*remote).await
                    }
                    Err(e) => Err(e),
                }
            }
        };
        match &result {
            Ok(_) => tracing::info!(session = %sid.0, "session finished"),
            Err(e) => tracing::info!(session = %sid.0, error = %e, "session ended with an error"),
        }
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
