//! Driving a paired device: our AI runs here, every tool call goes there as `call_tool`.
//!
//! A device keeps one cached connection. A call takes it out while it runs and puts it back
//! on success, so a cancelled or failed call drops the link (a half-read frame cannot be
//! resumed) and the next call dials again. A call whose request was never written is retried
//! once on a fresh link; one that failed after the write is not: it may have run.
//! All links share one endpoint (`cli.key`), bound on first use.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use data_encoding::BASE64;
use iroh::Endpoint;
use serde_json::Value;
use ywza_proto::{client, read_frame, write_frame, AgentMsg, ClientMsg, Control, ToolInfo};

use crate::{
    llm::{remote_system_prompt, Executor},
    tools::{monitor_line, parse_window, ToolOutput},
};

const CLIENT_NAME: &str = "ywza-agent";

struct Link {
    c: Control,
    next: u64,
}

impl Drop for Link {
    fn drop(&mut self) {
        self.c.conn.close(0u32.into(), b"bye");
    }
}

/// A failed call; `sent`: the request frame was written, so the device may have run it.
struct CallError {
    err: anyhow::Error,
    sent: bool,
}

impl Link {
    /// Dials the device on `ep`; also returns its OS from the Welcome.
    async fn dial(ep: &Endpoint, name: &str) -> Result<(Self, Option<String>)> {
        let (c, welcome) =
            tokio::time::timeout(Duration::from_secs(30), client::dial(ep, name, CLIENT_NAME))
                .await
                .map_err(|_| anyhow!("timed out connecting"))??;
        let os = match welcome {
            AgentMsg::Welcome { os, .. } => os,
            _ => None,
        };
        Ok((Self { c, next: 0 }, os))
    }

    fn alive(&self) -> bool {
        self.c.conn.close_reason().is_none()
    }

    /// The next message that is an error or that `want` accepts; the device also pushes
    /// events, cursors and images, which are skipped.
    async fn reply(&mut self, want: impl Fn(&AgentMsg) -> bool) -> Result<AgentMsg> {
        loop {
            let m = read_frame::<_, AgentMsg>(&mut self.c.recv).await?;
            if matches!(m, AgentMsg::Error { .. }) || want(&m) {
                return Ok(m);
            }
        }
    }

    async fn ask(&mut self, msg: ClientMsg, want: impl Fn(&AgentMsg) -> bool) -> Result<AgentMsg> {
        write_frame(&mut self.c.send, &msg).await?;
        match self.reply(want).await? {
            AgentMsg::Error { message } => bail!("{message}"),
            m => Ok(m),
        }
    }

    async fn call(&mut self, name: &str, args: &Value) -> Result<ToolOutput, CallError> {
        self.next += 1;
        let id = self.next;
        let msg = ClientMsg::CallTool {
            call: id,
            name: name.into(),
            args: args.clone(),
        };
        write_frame(&mut self.c.send, &msg)
            .await
            .map_err(|e| CallError {
                err: e.into(),
                sent: false,
            })?;
        let sent = |err: anyhow::Error| CallError { err, sent: true };
        match self
            .reply(|m| matches!(m, AgentMsg::ToolResult { call, .. } if *call == id))
            .await
            .map_err(sent)?
        {
            AgentMsg::ToolResult {
                ok,
                text,
                png_base64,
                ..
            } => {
                let supersedes_key = (name == "get_elements")
                    .then(|| args.get("window").and_then(parse_window))
                    .flatten()
                    .map(|w| format!("elements:w{w}"));
                Ok(ToolOutput {
                    text,
                    ok,
                    image_png: png_base64.and_then(|p| BASE64.decode(p.as_bytes()).ok()),
                    supersedes_key,
                })
            }
            AgentMsg::Error { message } => Ok(ToolOutput::err(message)),
            m => Err(sent(anyhow!("unexpected reply {m:?}"))),
        }
    }
}

#[derive(Default)]
struct State {
    link: Option<Link>,
    os: Option<String>,
    monitors: String,
    tools: Vec<ToolInfo>,
}

/// One paired device as a tool executor.
pub struct Remote {
    name: String,
    ep: Endpoint,
    state: Mutex<State>,
    /// Its link died or its device file is gone; `Remotes::open` replaces it.
    gone: AtomicBool,
}

impl Remote {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The device's OS and monitor lines, as of the last `Remotes::open`.
    pub fn describe(&self) -> (Option<String>, String) {
        let s = self.state.lock().unwrap();
        (s.os.clone(), s.monitors.clone())
    }

    fn take(&self) -> Option<Link> {
        self.state.lock().unwrap().link.take().filter(Link::alive)
    }

    fn put(&self, link: Link) {
        self.state.lock().unwrap().link = Some(link);
    }

    fn mark_gone(&self) {
        self.gone.store(true, Ordering::Relaxed);
    }

    async fn dial(&self) -> Result<Link> {
        tracing::debug!(device = %self.name, "dialing the device");
        let (link, os) = Link::dial(&self.ep, &self.name).await?;
        self.state.lock().unwrap().os = os;
        Ok(link)
    }

    /// The cached link, or a fresh one; fails when the device is no longer paired.
    async fn link(&self) -> Result<Link> {
        if !client::device_file(&self.name).exists() {
            self.state.lock().unwrap().link = None;
            self.mark_gone();
            bail!("no paired device `{}`", self.name);
        }
        match self.take() {
            Some(l) => Ok(l),
            None => self.dial().await,
        }
    }

    /// Connects if needed and refreshes the device's tools and monitors. A cached link that
    /// fails is dropped and the refresh retried once on a fresh one.
    async fn prepare(&self) -> Result<()> {
        let first = self.link().await?;
        if let Err(e) = self.refresh(first).await {
            tracing::debug!(device = %self.name, error = %e, "cached link failed, redialing");
            let fresh = self.dial().await;
            let r = match fresh {
                Ok(link) => self.refresh(link).await,
                Err(e) => Err(e),
            };
            if r.is_err() {
                self.mark_gone();
            }
            return r;
        }
        Ok(())
    }

    async fn refresh(&self, mut link: Link) -> Result<()> {
        let tools = match link
            .ask(ClientMsg::ListTools, |m| {
                matches!(m, AgentMsg::Tools { .. })
            })
            .await?
        {
            AgentMsg::Tools { list } => list,
            m => bail!("expected tools, got {m:?}"),
        };
        let monitors = match link
            .ask(ClientMsg::ListMonitors, |m| {
                matches!(m, AgentMsg::Monitors { .. })
            })
            .await
        {
            Ok(AgentMsg::Monitors { list }) => {
                list.iter().map(monitor_line).collect::<Vec<_>>().join("\n")
            }
            _ => "(monitors unavailable)".into(),
        };
        let mut s = self.state.lock().unwrap();
        s.tools = tools;
        s.monitors = monitors;
        s.link = Some(link);
        Ok(())
    }
}

impl Executor for Remote {
    fn tools(&self) -> Vec<ToolInfo> {
        self.state.lock().unwrap().tools.clone()
    }

    async fn system_prompt(&self) -> String {
        let s = self.state.lock().unwrap();
        remote_system_prompt(&self.name, s.os.as_deref(), &s.monitors)
    }

    async fn call(&self, _device: Option<&str>, name: &str, args: &Value) -> ToolOutput {
        let mut retried = false;
        loop {
            let mut link = match self.link().await {
                Ok(l) => l,
                Err(e) => return ToolOutput::err(format!("{}: {e:#}", self.name)),
            };
            match link.call(name, args).await {
                Ok(out) => {
                    self.put(link);
                    return out;
                }
                Err(CallError { sent: false, .. }) if !retried => retried = true,
                Err(CallError { err, sent }) => {
                    tracing::warn!(device = %self.name, error = %err, "remote tool call failed");
                    self.mark_gone();
                    let outcome = if sent {
                        "it may or may not have run"
                    } else {
                        "it did not run"
                    };
                    return ToolOutput::err(format!(
                        "{}: connection failed during the call ({err:#}); {outcome}",
                        self.name
                    ));
                }
            }
        }
    }
}

/// The agent's cached connections, by device name, over one shared endpoint.
#[derive(Default)]
pub struct Remotes {
    ep: tokio::sync::Mutex<Option<Endpoint>>,
    map: Mutex<HashMap<String, Arc<Remote>>>,
}

impl Remotes {
    async fn endpoint(&self, name: &str) -> Result<Endpoint> {
        let mut ep = self.ep.lock().await;
        if let Some(e) = ep.as_ref() {
            return Ok(e.clone());
        }
        let e = client::bind_devices(&[name]).await?;
        *ep = Some(e.clone());
        Ok(e)
    }

    /// The device ready to drive.
    pub async fn open(&self, name: &str) -> Result<Arc<Remote>> {
        if !client::valid_device_name(name) {
            bail!("bad device name `{name}`");
        }
        if !client::device_file(name).exists() {
            self.map.lock().unwrap().remove(name);
            bail!("no paired device `{name}`");
        }
        let cached = {
            let mut map = self.map.lock().unwrap();
            if map
                .get(name)
                .is_some_and(|r| r.gone.load(Ordering::Relaxed))
            {
                map.remove(name);
            }
            map.get(name).cloned()
        };
        let remote = match cached {
            Some(r) => r,
            None => {
                let ep = self.endpoint(name).await?;
                self.map
                    .lock()
                    .unwrap()
                    .entry(name.into())
                    .or_insert_with(|| {
                        Arc::new(Remote {
                            name: name.into(),
                            ep,
                            state: Mutex::default(),
                            gone: AtomicBool::new(false),
                        })
                    })
                    .clone()
            }
        };
        remote
            .prepare()
            .await
            .with_context(|| format!("device `{name}`"))?;
        Ok(remote)
    }
}
