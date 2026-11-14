use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use serde_json::Value;
use ywza_desktop::{Snapshot, WindowId};
use ywza_proto::{OcrEngine, SessionId, ToolInfo};
use ywza_term::TermManager;

use super::{act::Report, registry, Entry, ToolOutput};

/// One caller's element ids: a session, and for a remote caller its own session on the peer.
/// Each space keeps its own snapshots, so callers never overwrite each other's ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Space {
    pub session: SessionId,
    pub sub: Option<SessionId>,
}

type Snaps = HashMap<Space, HashMap<WindowId, (Instant, Snapshot)>>;

/// State shared by every call: element lists, terminals and the tool registry.
struct Shared {
    /// Last snapshot per window and space; element actions resolve ids against it.
    snaps: Mutex<Snaps>,
    terms: Arc<TermManager>,
    entries: Vec<Entry>,
}

/// What a tool run sees: the shared state plus this call's OCR engine and batch mode.
#[derive(Clone)]
pub struct Ctx {
    shared: Arc<Shared>,
    space: Space,
    /// The configured engine (get_elements and OCR validation).
    pub ocr: OcrEngine,
    /// Inside a `batch`: element actions skip the settle wait and the diff.
    pub quiet: bool,
}

impl Ctx {
    pub fn snap(&self, w: WindowId) -> Option<Snapshot> {
        let snaps = self.shared.snaps.lock().unwrap();
        Some(snaps.get(&self.space)?.get(&w)?.1.clone())
    }

    pub fn has_snap(&self, w: WindowId) -> bool {
        let snaps = self.shared.snaps.lock().unwrap();
        snaps.get(&self.space).is_some_and(|m| m.contains_key(&w))
    }

    pub fn store(&self, w: WindowId, snap: Snapshot) {
        self.shared.store(self.space, w, snap);
    }

    pub fn forget(&self, w: WindowId) {
        if let Some(m) = self.shared.snaps.lock().unwrap().get_mut(&self.space) {
            m.remove(&w);
        }
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

impl Shared {
    fn store(&self, space: Space, w: WindowId, snap: Snapshot) {
        let mut snaps = self.snaps.lock().unwrap();
        snaps
            .entry(space)
            .or_default()
            .insert(w, (Instant::now(), snap));
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

    /// The window's element list stored in `prefer`, else the most recent across all spaces. A
    /// stale one (window rect not `rect`) is re-taken with its ids carried over and stored back in
    /// its space; with none stored, a fresh one is taken and not stored.
    pub async fn latest_snapshot(
        &self,
        prefer: Option<Space>,
        w: WindowId,
        rect: ywza_desktop::Rect,
    ) -> ywza_desktop::Result<Snapshot> {
        let latest = {
            let snaps = self.shared.snaps.lock().unwrap();
            let own = prefer.and_then(|sp| snaps.get(&sp)?.get(&w).map(|(_, s)| (sp, s.clone())));
            own.or_else(|| {
                snaps
                    .iter()
                    .filter_map(|(sp, m)| m.get(&w).map(|(t, s)| (*sp, *t, s)))
                    .max_by_key(|(_, t, _)| *t)
                    .map(|(sp, _, s)| (sp, s.clone()))
            })
        };
        let cached = latest.as_ref().map(|l| l.1.clone());
        if let Some(s) = cached.as_ref().filter(|s| s.window.rect == rect) {
            return Ok(s.clone());
        }
        let s = super::util::blocking(move || ywza_desktop::snapshot(w, cached.as_ref())).await?;
        if let Some((space, _)) = latest {
            self.shared.store(space, w, s.clone());
        }
        Ok(s)
    }

    /// The window's stored snapshot in `space`, if any.
    pub fn snap(&self, space: Space, w: WindowId) -> Option<Snapshot> {
        let snaps = self.shared.snaps.lock().unwrap();
        Some(snaps.get(&space)?.get(&w)?.1.clone())
    }

    /// Stores `snap` as the window's latest in `space`; element ids resolve against it.
    pub fn store(&self, space: Space, w: WindowId, snap: Snapshot) {
        self.shared.store(space, w, snap);
    }

    pub fn terms(&self) -> Arc<TermManager> {
        self.shared.terms.clone()
    }

    /// Forgets every element space of the session, remote sub-spaces included.
    pub fn drop_session(&self, session: SessionId) {
        self.shared
            .snaps
            .lock()
            .unwrap()
            .retain(|sp, _| sp.session != session);
    }

    /// Runs a tool in an element space. `ocr` is the configured engine.
    pub async fn call(
        &self,
        space: Space,
        name: &str,
        args: &Value,
        ocr: OcrEngine,
    ) -> ToolOutput {
        let cx = Ctx {
            shared: self.shared.clone(),
            space,
            ocr,
            quiet: false,
        };
        cx.call(name, args.clone()).await
    }
}
