use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use serde_json::Value;
use ywza_desktop::{Snapshot, WindowId};
use ywza_proto::{OcrEngine, ToolInfo};
use ywza_term::TermManager;

use super::{act::Report, registry, Entry, ToolOutput};

/// State shared by every call: element lists, terminals and the tool registry.
struct Shared {
    /// Last snapshot per window; element actions resolve ids against it.
    snaps: Mutex<HashMap<WindowId, Snapshot>>,
    terms: Arc<TermManager>,
    entries: Vec<Entry>,
}

/// What a tool run sees: the shared state plus this call's OCR engine and batch mode.
#[derive(Clone)]
pub struct Ctx {
    shared: Arc<Shared>,
    /// The configured engine (get_elements and OCR validation).
    pub ocr: OcrEngine,
    /// Inside a `batch`: element actions skip the settle wait and the diff.
    pub quiet: bool,
}

impl Ctx {
    pub fn snap(&self, w: WindowId) -> Option<Snapshot> {
        self.shared.snaps.lock().unwrap().get(&w).cloned()
    }

    pub fn has_snap(&self, w: WindowId) -> bool {
        self.shared.snaps.lock().unwrap().contains_key(&w)
    }

    pub fn store(&self, w: WindowId, snap: Snapshot) {
        self.shared.snaps.lock().unwrap().insert(w, snap);
    }

    pub fn forget(&self, w: WindowId) {
        self.shared.snaps.lock().unwrap().remove(&w);
    }

    pub fn terms(&self) -> Arc<TermManager> {
        self.shared.terms.clone()
    }

    pub fn report(&self, mode: Report) -> Report {
        if self.quiet {
            Report::Quiet
        } else {
            mode
        }
    }

    pub fn quieter(&self) -> Self {
        Self {
            quiet: true,
            ..self.clone()
        }
    }

    /// Runs a registered tool by name; unknown names and bad arguments are error outputs.
    pub async fn call(&self, name: &str, args: Value) -> ToolOutput {
        let Some(e) = self.shared.entries.iter().find(|e| e.name == name) else {
            tracing::warn!(tool = name, "unknown tool");
            return ToolOutput::err(format!("unknown tool `{name}`"));
        };
        let start = Instant::now();
        let out = (e.call)(args, self).await;
        tracing::debug!(
            tool = name,
            ms = start.elapsed().as_millis() as u64,
            ok = out.ok,
            quiet = self.quiet,
            "tool call"
        );
        out
    }
}

pub struct ToolSpec {
    pub name: &'static str,
    pub risky: bool,
    pub description: String,
    pub parameters: Value,
}

/// The agent's tools: a registry plus the state their calls share.
pub struct Tools {
    shared: Arc<Shared>,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared {
                snaps: Mutex::default(),
                terms: Arc::new(TermManager::new()),
                entries: registry(),
            }),
        }
    }
}

impl Tools {
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.shared
            .entries
            .iter()
            .map(|e| ToolSpec {
                name: e.name,
                risky: e.risky,
                description: e.description.clone(),
                parameters: e.schema.clone(),
            })
            .collect()
    }

    /// Whether a call to `name` asks when no approval rule matches. Unknown tools ask.
    pub fn risky(&self, name: &str) -> bool {
        self.shared
            .entries
            .iter()
            .find(|e| e.name == name)
            .is_none_or(|e| e.risky)
    }

    /// The tools as clients and models see them.
    pub fn infos(&self) -> Vec<ToolInfo> {
        self.specs()
            .into_iter()
            .map(|s| ToolInfo {
                name: s.name.into(),
                risky: s.risky,
                description: s.description,
                parameters: s.parameters,
            })
            .collect()
    }

    /// The window's latest element list (the one `get_elements` ids refer to), kept as the
    /// latest. A stored one whose window rect is not `rect` is stale: re-taken with its ids carried over.
    pub async fn latest_snapshot(
        &self,
        w: WindowId,
        rect: ywza_desktop::Rect,
    ) -> ywza_desktop::Result<Snapshot> {
        let cached = self.shared.snaps.lock().unwrap().get(&w).cloned();
        if let Some(s) = cached.as_ref().filter(|s| s.window.rect == rect) {
            return Ok(s.clone());
        }
        let s = super::util::blocking(move || ywza_desktop::snapshot(w, cached.as_ref())).await?;
        self.shared.snaps.lock().unwrap().insert(w, s.clone());
        Ok(s)
    }

    /// Runs a tool. `ocr` is the configured engine.
    pub async fn call(&self, name: &str, args: &Value, ocr: OcrEngine) -> ToolOutput {
        let cx = Ctx {
            shared: self.shared.clone(),
            ocr,
            quiet: false,
        };
        cx.call(name, args.clone()).await
    }
}
