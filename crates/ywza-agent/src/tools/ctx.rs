use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use ywza_desktop::{Snapshot, WindowId};
use ywza_proto::OcrEngine;
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
        match self.shared.entries.iter().find(|e| e.name == name) {
            Some(e) => (e.call)(args, self).await,
            None => ToolOutput::err(format!("unknown tool `{name}`")),
        }
    }
}

pub struct ToolSpec {
    pub name: &'static str,
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
                description: e.description.clone(),
                parameters: e.schema.clone(),
            })
            .collect()
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
