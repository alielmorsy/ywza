//! One connected client's messages: live view, input, pairing, tool calls, file copy.

use std::{future::Future, sync::Arc};

use iroh::endpoint::Connection;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use ywza_proto::{AgentMsg, ClientMsg, InputEvent, MouseButton, SessionId};

use super::{
    peer,
    xfer::{self, Pushes},
};
use crate::{
    live::{lens_act, to_pixels, Lens, Live},
    mcp,
    net::pair,
    tools::parse_window,
    Agent,
};

pub struct Client {
    agent: Arc<Agent>,
    conn: Connection,
    name: String,
    /// This machine's own app: may list models and run sessions on paired devices.
    local: bool,
    direct: UnboundedSender<AgentMsg>,
    /// Dropped (stopping capture) on live_stop, a new live_start, losing `view`, or disconnect.
    live: Option<Live>,
    /// Dropped on lens_stop, a new lens_start, losing `view`, or disconnect.
    lens: Option<Lens>,
    /// Created on the first `call_tool`, so its calls show up as one session.
    mcp: Option<SessionId>,
    /// The last normalized pointer position from `MouseMove`, for mapping taps to elements.
    pointer: (f32, f32),
    pub pushes: Pushes,
}

impl Client {
    pub fn new(
        agent: Arc<Agent>,
        conn: Connection,
        name: String,
        local: bool,
        direct: UnboundedSender<AgentMsg>,
    ) -> Self {
        Self {
            agent,
            conn,
            name,
            local,
            direct,
            live: None,
            lens: None,
            mcp: None,
            pointer: (0.0, 0.0),
            pushes: Pushes::default(),
        }
    }

    /// Stops everything that shows the screen (live view and lens).
    pub fn stop_view(&mut self) {
        self.live = None;
        self.lens = None;
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
            ClientMsg::LensStart { window } => self.lens_start(window),
            ClientMsg::LensStop => self.lens = None,
            ClientMsg::LensAct {
                call,
                window,
                element,
                text,
                submit,
            } => match &self.lens {
                Some(l) => {
                    let (agent, space) = (self.agent.clone(), l.space());
                    self.reply_later(async move {
                        lens_act(&agent, space, call, window, element, text, submit).await
                    });
                }
                None => self.send(AgentMsg::ToolResult {
                    call,
                    ok: false,
                    text: "lens is not running; send lens_start first".into(),
                    png_base64: None,
                }),
            },
            ClientMsg::CreatePairing => {
                let agent = self.agent.clone();
                self.reply_later(async move { pair::create(&agent).await });
            }
            ClientMsg::CallTool {
                call,
                name,
                args,
                space,
            } => {
                let agent = self.agent.clone();
                let sid = *self
                    .mcp
                    .get_or_insert_with(|| mcp::session(&agent, &self.name));
                self.reply_later(
                    async move { mcp::call_tool(&agent, sid, space, call, name, args).await },
                );
            }
            ClientMsg::Approve {
                request,
                allow,
                remember,
            } => {
                let may_edit = self.local
                    || peer::permissions(&self.agent, self.conn.remote_id())
                        .is_some_and(|p| p.settings);
                if let Some(reply) = self.agent.approve(request, allow, remember && may_edit) {
                    self.send(reply);
                }
            }
            ClientMsg::Pull {
                xfer,
                path,
                recursive,
            } => {
                tokio::spawn(xfer::pull(
                    self.conn.clone(),
                    xfer,
                    path,
                    recursive,
                    self.direct.clone(),
                ));
            }
            ClientMsg::Push { xfer, dest, name } => {
                self.send(self.pushes.start(xfer, &dest, &name))
            }
            ClientMsg::PushEnd { xfer, files } => {
                if let Some(reply) = self.pushes.end(xfer, files) {
                    self.send(reply);
                }
            }
            m
            @ (ClientMsg::ListMonitors | ClientMsg::ListModels | ClientMsg::WindowInfo { .. }) => {
                let (agent, local) = (self.agent.clone(), self.local);
                self.reply_later(async move { agent.inspect(m, local).await });
            }
            ClientMsg::Screenshot { target, max_width } => {
                let agent = self.agent.clone();
                self.reply_later(async move { agent.screenshot(target, max_width).await });
            }
            msg => {
                for reply in self.agent.handle(msg, self.local) {
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
            Err(e) => {
                tracing::error!(client = %self.name, monitor, error = %e, "live view failed to start");
                self.send(AgentMsg::Error {
                    message: format!("live view: {e}"),
                })
            }
        }
    }

    fn lens_start(&mut self, window: Option<String>) {
        self.lens = None;
        let target = match window {
            Some(w) => match parse_window(&Value::String(w.clone())) {
                Some(id) => Some(id),
                None => {
                    return self.send(AgentMsg::Error {
                        message: format!("lens: bad window id `{w}`"),
                    })
                }
            },
            None => None,
        };
        let monitor = self.live.as_ref().map(|l| l.monitor);
        self.lens = Some(Lens::start(
            self.agent.clone(),
            target,
            monitor,
            self.direct.clone(),
        ));
    }

    fn input(&mut self, event: InputEvent) {
        match (&event, &self.live) {
            (InputEvent::MouseMove { x, y }, _) => self.pointer = (*x, *y),
            (
                InputEvent::MouseButton {
                    button: MouseButton::Left,
                    down: true,
                },
                Some(l),
            ) => {
                let (x, y) = to_pixels(l.rect, self.pointer.0, self.pointer.1);
                self.agent.sos.touch(x, y);
            }
            _ => {}
        }
        match &self.live {
            Some(l) => l.input(event),
            None if !matches!(event, InputEvent::MouseMove { .. }) => self.send(AgentMsg::Error {
                message: "input: live view is not running".into(),
            }),
            None => {}
        }
    }
}
