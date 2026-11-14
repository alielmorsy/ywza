use std::{path::PathBuf, sync::Arc};

use iroh::{Endpoint, EndpointId};
use serde_json::Value;
use tokio::sync::broadcast;
use ywza_proto::{AgentMsg, SessionEvent, SessionId, Ticket};

use crate::{
    approval::Pending,
    atomic::write_atomic,
    config::ConfigStore,
    local, net,
    remote::Remotes,
    sessions::Sessions,
    tools::{Space, ToolOutput, Tools},
};

pub struct Agent {
    pub dir: PathBuf,
    pub config: ConfigStore,
    pub sessions: Sessions,
    pub tools: Tools,
    pub remotes: Remotes,
    pub approvals: Pending,
    /// Sessions paused on a blocker, waiting for the user.
    pub sos: crate::sos::Sos,
    pub pairing: net::pair::Pairing,
    pub tx: broadcast::Sender<AgentMsg>,
    pub http: reqwest::Client,
    pub endpoint: Endpoint,
    /// Implicitly trusted with every permission (`local-client.key`), never written into `peers`.
    pub local_client: EndpointId,
    /// Loopback only, no relays (tests, offline use).
    pub local_only: bool,
}

impl Agent {
    /// Loads config and keys from `dir`, binds the endpoint, writes `agent.ticket` and starts serving.
    /// `local_only` binds 127.0.0.1 with relays disabled (tests, offline use).
    pub async fn start(dir: PathBuf, local_only: bool) -> anyhow::Result<Arc<Self>> {
        // reqwest is built without a bundled provider; iroh already links ring.
        let _ = rustls::crypto::ring::default_provider().install_default();
        std::fs::create_dir_all(&dir)?;
        crate::vault::init(&dir);
        crate::labels::init(&dir);
        crate::plans::init(&dir);
        let watching = crate::watch::init(&dir);
        let config = ConfigStore::load(dir.join("config.toml"))?;
        let key = ywza_proto::load_or_create_secret_key(&dir.join("agent.key"))?;
        let local_client =
            ywza_proto::load_or_create_secret_key(&dir.join("local-client.key"))?.public();
        let relay = config.get().relay_url;
        let endpoint = net::bind(key, local_only, &dir, relay.as_deref()).await?;
        let ticket = Ticket {
            addr: net::local_addr(&endpoint),
        };
        write_atomic(&dir.join("agent.ticket"), &ticket.to_string())?;
        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            dir = %dir.display(),
            id = %endpoint.id(),
            port = endpoint.bound_sockets().iter().find(|a| a.is_ipv4()).map(|a| a.port()),
            relay = relay.as_deref().unwrap_or("default"),
            local_only,
            "agent listening"
        );

        let agent = Arc::new(Self {
            sessions: Sessions::new(dir.join("logs")),
            dir,
            config,
            tools: Default::default(),
            remotes: Default::default(),
            approvals: Default::default(),
            sos: Default::default(),
            pairing: Default::default(),
            tx: broadcast::channel(1024).0,
            http: reqwest::Client::new(),
            endpoint,
            local_client,
            local_only,
        });
        tokio::spawn(net::watch_relay(agent.endpoint.clone()));
        if watching {
            crate::watch::spawn(agent.clone());
        }
        tokio::spawn(net::accept_loop(agent.clone()));
        tokio::spawn(net::watch_pairing_requests(agent.clone()));
        match local::bind(&agent.dir).await {
            Ok(listener) => {
                tracing::info!(
                    port = listener.0.local_addr().map_or(0, |a| a.port()),
                    "local TCP port open"
                );
                drop(tokio::spawn(local::accept_loop(agent.clone(), listener)))
            }
            Err(e) => tracing::error!(error = %e, "local TCP port unavailable"),
        }
        Ok(agent)
    }

    pub fn broadcast(&self, msg: AgentMsg) {
        let _ = self.tx.send(msg); // Err only means no client is connected.
    }

    /// Records a session event and sends it to every client.
    pub fn emit(&self, session: SessionId, event: SessionEvent) {
        self.sessions.push(session, &event);
        self.broadcast(AgentMsg::Event { session, event });
    }

    /// Logs a tool call; `notify` also shows its text as an assistant message.
    pub fn emit_tool_call(
        &self,
        sid: SessionId,
        id: &str,
        name: &str,
        args: &Value,
        device: Option<&str>,
    ) {
        self.emit(
            sid,
            SessionEvent::ToolCall {
                id: id.into(),
                name: name.into(),
                args: args.clone(),
                device: device.map(String::from),
            },
        );
        if name == "notify" {
            let text = args["text"].as_str().unwrap_or_default().to_string();
            self.emit(sid, SessionEvent::Assistant { text });
        }
    }

    /// `name`: the tool; `show_screenshot`'s image goes to the user in the event, never to the model.
    pub fn emit_tool_result(&self, sid: SessionId, id: &str, name: &str, out: &ToolOutput) {
        let shown = name == crate::tools::SHOW_SCREENSHOT;
        self.emit(
            sid,
            SessionEvent::ToolResult {
                id: id.into(),
                ok: out.ok,
                text: out.text.clone(),
                has_image: out.image_png.is_some() && !shown,
                image: out
                    .image_png
                    .as_ref()
                    .filter(|_| shown)
                    .map(|p| data_encoding::BASE64.encode(p)),
            },
        );
    }

    /// Runs a tool with the configured input idle time and OCR engine. Vault placeholders in
    /// typed text are filled in first and vault values in the output are hidden again.
    pub async fn run_tool(&self, space: Space, name: &str, args: &Value) -> ToolOutput {
        let cfg = self.config.get();
        ywza_desktop::set_idle_before_input(cfg.desktop.idle_before_input_ms.into());
        // Taken before the call: it reads the element list the call's ids refer to.
        let rec = crate::plans::record::before(&self.tools, space, name, args);
        let args = match crate::vault::substitute(name, args) {
            Ok(a) => a,
            Err(e) => return ToolOutput::err(e),
        };
        let mut out = self.tools.call(space, name, &args, cfg.ocr.engine).await;
        crate::vault::scrub(&mut out.text);
        if let Some(rec) = rec {
            crate::plans::record::after(rec, &out);
        }
        out
    }

    /// A client's `screenshot` request: no marks, sent only to that client. Full resolution
    /// unless `max_width` asks for a smaller one.
    pub async fn screenshot(&self, target: ywza_proto::Target, max_width: Option<u32>) -> AgentMsg {
        let png = crate::tools::capture(&target).await.and_then(|img| {
            let img = match max_width.filter(|&w| w > 0 && w < img.width) {
                // `downscale` caps the long side; scale that cap so the width lands on `w`.
                Some(w) => {
                    let long = img.width.max(img.height) as u64;
                    let cap = (long * w as u64 / img.width as u64) as u32;
                    crate::tools::downscale(img, cap)
                }
                None => img,
            };
            Ok((ywza_desktop::encode_png(&img)?, img.width, img.height))
        });
        match png {
            Ok((png, width, height)) => AgentMsg::Image {
                png_base64: data_encoding::BASE64.encode(&png),
                width,
                height,
            },
            Err(e) => AgentMsg::Error {
                message: format!("screenshot: {e}"),
            },
        }
    }
}
