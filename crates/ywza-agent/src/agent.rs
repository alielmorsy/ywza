use std::{path::PathBuf, sync::Arc};

use iroh::{Endpoint, EndpointId};
use serde_json::Value;
use tokio::sync::broadcast;
use ywza_proto::{AgentMsg, SessionEvent, SessionId, Ticket};

use crate::{
    atomic::write_atomic,
    config::ConfigStore,
    local, net,
    sessions::Sessions,
    tools::{ToolOutput, Tools},
};

pub struct Agent {
    pub dir: PathBuf,
    pub config: ConfigStore,
    pub sessions: Sessions,
    pub tools: Tools,
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
        let config = ConfigStore::load(dir.join("config.toml"))?;
        let key = ywza_proto::load_or_create_secret_key(&dir.join("agent.key"))?;
        let local_client =
            ywza_proto::load_or_create_secret_key(&dir.join("local-client.key"))?.public();
        let endpoint = net::bind(key, local_only, &dir).await?;
        let ticket = Ticket {
            addr: net::local_addr(&endpoint),
        };
        write_atomic(&dir.join("agent.ticket"), &ticket.to_string())?;

        let agent = Arc::new(Self {
            sessions: Sessions::new(dir.join("logs")),
            dir,
            config,
            tools: Default::default(),
            pairing: Default::default(),
            tx: broadcast::channel(1024).0,
            http: reqwest::Client::new(),
            endpoint,
            local_client,
            local_only,
        });
        tokio::spawn(net::accept_loop(agent.clone()));
        tokio::spawn(net::watch_pairing_requests(agent.clone()));
        match local::bind(&agent.dir).await {
            Ok(listener) => drop(tokio::spawn(local::accept_loop(agent.clone(), listener))),
            Err(e) => eprintln!("ywza-agent: local TCP port: {e}"),
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
    pub fn emit_tool_call(&self, sid: SessionId, id: &str, name: &str, args: &Value) {
        self.emit(
            sid,
            SessionEvent::ToolCall {
                id: id.into(),
                name: name.into(),
                args: args.clone(),
            },
        );
        if name == "notify" {
            let text = args["text"].as_str().unwrap_or_default().to_string();
            self.emit(sid, SessionEvent::Assistant { text });
        }
    }

    pub fn emit_tool_result(&self, sid: SessionId, id: &str, out: &ToolOutput) {
        self.emit(
            sid,
            SessionEvent::ToolResult {
                id: id.into(),
                ok: out.ok,
                text: out.text.clone(),
                has_image: out.image_png.is_some(),
            },
        );
    }

    /// Runs a tool with the configured input idle time and OCR engine.
    pub async fn run_tool(&self, name: &str, args: &Value) -> ToolOutput {
        let cfg = self.config.get();
        ywza_desktop::set_idle_before_input(cfg.desktop.idle_before_input_ms.into());
        self.tools.call(name, args, cfg.ocr.engine).await
    }

    /// A client's `screenshot` request: full resolution, no marks, sent only to that client.
    pub async fn screenshot(&self, target: ywza_proto::Target) -> AgentMsg {
        let png = crate::tools::capture(&target)
            .await
            .and_then(|img| Ok((ywza_desktop::encode_png(&img)?, img.width, img.height)));
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
