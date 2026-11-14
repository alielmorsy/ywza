use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use ywza_proto::{client, AgentMsg, ApprovalMode, ClientMsg, DeviceInfo, SessionEvent, SessionId};

use crate::{llm, remote::Remote, Agent};

/// How one prompt runs: on which device (None: this machine), which other devices the AI may
/// use, and with which overrides.
struct Run {
    device: Option<String>,
    devices: Vec<String>,
    opts: llm::Opts,
}

impl Agent {
    /// Handles one client message; the return value goes back to that client only.
    /// `local`: the client is this machine's own app. Peers need `drive` for sessions on devices;
    /// the permission gate checks that before a message gets here.
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
                devices,
                model,
                effort,
                approval,
            } => {
                let sid = session.unwrap_or_default();
                if device.is_some() && !devices.is_empty() {
                    return err("use either device or devices, not both");
                }
                let paired = client::list_devices();
                if let Some(bad) = devices.iter().find(|d| !paired.contains(d)) {
                    return err(&format!("no paired device `{bad}`"));
                }
                match self.sessions.target(sid) {
                    Some((d, ds)) if d != device || ds != devices => {
                        return err("the session runs on different devices");
                    }
                    Some(_) => {}
                    None => self
                        .sessions
                        .create_with(sid, &text, device.as_deref(), &devices),
                }
                let (model, effort) = if local { (model, effort) } else { (None, None) };
                match approval {
                    Some(mode) if local => self.sessions.set_approval(sid, mode),
                    // A peer cannot send a mode; its sessions still ask for risky tools.
                    _ if !local => self.sessions.set_approval(sid, ApprovalMode::Rules),
                    _ => {}
                }
                match self.sessions.begin(sid) {
                    Ok((cancel, history)) => {
                        let run = Run {
                            device,
                            devices,
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
            // The saver gets the stored config through the broadcast, like every other client.
            ClientMsg::SetConfig { config } => match self.config.set(config) {
                Ok(()) => {
                    tracing::debug!("settings changed by a client");
                    self.broadcast(AgentMsg::Config {
                        config: self.config.get().masked(),
                    });
                    None
                }
                Err(e) => {
                    tracing::error!(error = %e, "saving config failed");
                    err(&format!("saving config: {e}"))
                }
            },
            ClientMsg::ListDevices => Some(AgentMsg::Devices {
                list: client::list_devices()
                    .into_iter()
                    .filter_map(|name| {
                        let (ticket, _) = client::load_device(&name).ok()?;
                        Some(DeviceInfo {
                            name,
                            node_id: ticket.addr.id.to_string(),
                        })
                    })
                    .collect(),
            }),
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

    async fn open_remote(
        &self,
        name: &str,
        cancel: &CancellationToken,
    ) -> Result<Arc<Remote>, String> {
        tokio::select! {
            r = self.remotes.open(name) => r.map_err(|e| format!("{e:#}")),
            _ = cancel.cancelled() => Err("cancelled".to_string()),
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
        let result = match (&run.device, run.devices.is_empty()) {
            (None, true) => {
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
            (None, false) => {
                let mut devices = Vec::new();
                let mut failed = None;
                for name in &run.devices {
                    match self.open_remote(name, &cancel).await {
                        Ok(r) => devices.push(r),
                        Err(e) => {
                            failed = Some(e);
                            break;
                        }
                    }
                }
                match failed {
                    None => {
                        let multi = llm::Multi {
                            agent: &*self,
                            devices,
                        };
                        llm::run(&self, sid, &mut history, &cancel, &run.opts, &multi).await
                    }
                    Some(e) => Err(e),
                }
            }
            (Some(device), _) => match self.open_remote(device, &cancel).await {
                Ok(remote) => {
                    llm::run(&self, sid, &mut history, &cancel, &run.opts, &*remote).await
                }
                Err(e) => Err(e),
            },
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
