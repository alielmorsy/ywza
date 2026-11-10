//! Read-only queries for the apps: monitors, window geometry with element boxes, model ids.

use std::time::Duration;

use serde_json::Value;
use ywza_desktop as desk;
use ywza_proto::{AgentMsg, ClientMsg, ElementInfo, MonitorInfo, ScreenRect};

use crate::{
    tools::{blocking, monitor_info, parse_window},
    Agent,
};

fn error(message: impl Into<String>) -> AgentMsg {
    AgentMsg::Error {
        message: message.into(),
    }
}

impl Agent {
    /// Answers `ListMonitors`, `ListModels` and `WindowInfo`. `local`: the client is this
    /// machine's own app (model ids reveal what the API key can use).
    pub async fn inspect(&self, msg: ClientMsg, local: bool) -> AgentMsg {
        match msg {
            ClientMsg::ListMonitors => match monitors().await {
                Ok(list) => AgentMsg::Monitors { list },
                Err(e) => error(format!("monitors: {e}")),
            },
            ClientMsg::ListModels if local => match self.models().await {
                Ok(list) => AgentMsg::Models { list },
                Err(e) => error(e),
            },
            ClientMsg::ListModels => error("models are for local clients only"),
            ClientMsg::WindowInfo { window, elements } => self.window_info(window, elements).await,
            _ => error("not an inspect message"),
        }
    }

    /// `GET {base_url}/models` with the configured key; ids sorted.
    async fn models(&self) -> Result<Vec<String>, String> {
        let llm = self.config.get().llm;
        let url = format!("{}/models", llm.base_url.trim_end_matches('/'));
        let mut req = self.http.get(url).timeout(Duration::from_secs(15));
        if !llm.api_key.is_empty() {
            req = req.bearer_auth(&llm.api_key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("models request failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("models response: {e}"))?;
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(format!("models: {status}: {snippet}"));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("models response: {e}"))?;
        let mut ids: Vec<String> = v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["id"].as_str().map(String::from))
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    async fn window_info(&self, window: String, elements: bool) -> AgentMsg {
        let Some(id) = parse_window(&Value::String(window.clone())) else {
            return error(format!("bad window id `{window}`"));
        };
        let found = blocking(move || {
            let win = desk::list_windows()?.into_iter().find(|w| w.id == id);
            Ok::<_, desk::Error>((win, desk::list_monitors()?))
        })
        .await;
        let (win, mons) = match found {
            Ok((Some(w), m)) => (w, m),
            Ok((None, _)) => return error(format!("unknown window {window}")),
            Err(e) => return error(format!("window_info: {e}")),
        };
        // Element boxes and the window rect must come from the same moment.
        let snap = if elements {
            match self.tools.latest_snapshot(id, win.rect).await {
                Ok(snap) => Some(snap),
                Err(e) => return error(format!("window_info: {e}")),
            }
        } else {
            None
        };
        let (rect, monitor) = snap.as_ref().map_or((win.rect, win.monitor), |s| {
            (s.window.rect, s.window.monitor)
        });
        let (ox, oy) = mons
            .iter()
            .find(|m| m.index == monitor)
            .map_or((0, 0), |m| (m.rect.x, m.rect.y));
        let rel = |r: desk::Rect| ScreenRect {
            x: r.x - ox,
            y: r.y - oy,
            width: r.w.max(0) as u32,
            height: r.h.max(0) as u32,
        };
        let elements = snap.map(|snap| {
            snap.elements
                .iter()
                .map(|e| ElementInfo {
                    id: e.id,
                    role: e.kind.into(),
                    name: e.name.clone(),
                    rect: rel(e.rect),
                })
                .collect()
        });
        AgentMsg::WindowInfo {
            window,
            title: win.title,
            monitor: monitor as u32,
            rect: rel(rect),
            elements,
        }
    }
}

async fn monitors() -> desk::Result<Vec<MonitorInfo>> {
    Ok(blocking(desk::list_monitors)
        .await?
        .iter()
        .map(monitor_info)
        .collect())
}
