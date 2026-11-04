//! One connected client's messages: live view, input, pairing, tool calls.

use std::{future::Future, sync::Arc};

use iroh::endpoint::Connection;
use tokio::sync::mpsc::UnboundedSender;
use ywza_proto::{AgentMsg, ClientMsg, InputEvent, SessionId};

use crate::{live::Live, mcp, net::pair, Agent};

pub struct Client {
    agent: Arc<Agent>,
    conn: Connection,
    name: String,
    direct: UnboundedSender<AgentMsg>,
    /// Dropped (stopping capture) on live_stop, a new live_start, losing `view`, or disconnect.
    live: Option<Live>,
    /// Created on the first `call_tool`, so its calls show up as one session.
    mcp: Option<SessionId>,
}

impl Client {
    pub fn new(
        agent: Arc<Agent>,
        conn: Connection,
        name: String,
        direct: UnboundedSender<AgentMsg>,
    ) -> Self {
        Self {
            agent,
            conn,
            name,
            direct,
            live: None,
            mcp: None,
        }
    }

    pub fn stop_live(&mut self) {
        self.live = None;
    }

    /// Ends the client's tool-call session, if it made calls.
    pub fn finish(self) {
        if let Some(sid) = self.mcp {
            mcp::end_session(&self.agent, sid);
        }
    }

    fn send(&self, msg: AgentMsg) {
        let _ = self.direct.send(msg);
    }

    fn reply_later(&self, reply: impl Future<Output = AgentMsg> + Send + 'static) {
        let direct = self.direct.clone();
        tokio::spawn(async move {
            let _ = direct.send(reply.await);
        });
    }

    /// Permissions were checked by the caller.
    pub fn handle(&mut self, msg: ClientMsg) {
        match msg {
            ClientMsg::LiveStart { monitor } => self.live_start(monitor),
            ClientMsg::LiveStop => self.live = None,
            ClientMsg::Input { event } => self.input(event),
            ClientMsg::CreatePairing => {
                let agent = self.agent.clone();
                self.reply_later(async move { pair::create(&agent).await });
            }
            ClientMsg::CallTool { call, name, args } => {
                let agent = self.agent.clone();
                let sid = *self
                    .mcp
                    .get_or_insert_with(|| mcp::session(&agent, &self.name));
                self.reply_later(async move { mcp::call_tool(&agent, sid, call, name, args).await });
            }
            ClientMsg::Screenshot { target } => {
                let agent = self.agent.clone();
                self.reply_later(async move { agent.screenshot(target).await });
            }
            msg => {
                if let Some(reply) = self.agent.handle(msg) {
                    self.send(reply);
                }
            }
        }
    }

    fn live_start(&mut self, monitor: u32) {
        self.live = None;
        let s = self.agent.config.get().stream;
        let fixed_bps = (s.bitrate_kbps > 0).then(|| s.bitrate_kbps.saturating_mul(1000));
        let fps = s.fps.clamp(1, 60);
        let started = Live::start(
            self.conn.clone(),
            monitor,
            fps,
            fixed_bps,
            s.cursor,
            self.direct.clone(),
        );
        match started {
            Ok(l) => self.live = Some(l),
            Err(e) => self.send(AgentMsg::Error {
                message: format!("live view: {e}"),
            }),
        }
    }

    fn input(&mut self, event: InputEvent) {
        let r = match &self.live {
            Some(l) => l.input(&event),
            None => Err(anyhow::anyhow!("live view is not running")),
        };
        // A failing move would repeat at mouse rate; its button/key events report it.
        let is_move = matches!(event, InputEvent::MouseMove { .. });
        if let (Err(e), false) = (r, is_move) {
            self.send(AgentMsg::Error {
                message: format!("input: {e}"),
            });
        }
    }
}
