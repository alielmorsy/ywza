//! Session store: in memory, plus `<config_dir>/logs/<session>.jsonl` activity logs.

use std::{
    collections::HashMap,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use tokio_util::sync::CancellationToken;
use ywza_proto::{SessionEvent, SessionId, SessionStatus, SessionSummary};

use crate::llm::Msg;

struct Session {
    summary: SessionSummary,
    events: Vec<SessionEvent>,
    history: Vec<Msg>,
    /// Some while the loop runs.
    cancel: Option<CancellationToken>,
}

pub struct Sessions {
    logs: PathBuf,
    map: Mutex<HashMap<SessionId, Session>>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Sessions {
    pub fn new(logs: PathBuf) -> Self {
        Self {
            logs,
            map: Mutex::default(),
        }
    }

    pub fn create(&self, prompt: &str) -> SessionId {
        let id = SessionId::new();
        let mut title: String = prompt.chars().take(60).collect();
        if title.len() < prompt.len() {
            title.push('…');
        }
        let s = Session {
            summary: SessionSummary {
                id,
                title,
                status: SessionStatus::Running,
                updated_ms: now_ms(),
            },
            events: Vec::new(),
            history: Vec::new(),
            cancel: None,
        };
        self.map.lock().unwrap().insert(id, s);
        id
    }

    /// Marks the session running and hands out its history. Err if unknown or already running.
    pub fn begin(&self, id: SessionId) -> Result<(CancellationToken, Vec<Msg>), &'static str> {
        let mut map = self.map.lock().unwrap();
        let s = map.get_mut(&id).ok_or("unknown session")?;
        if s.cancel.is_some() {
            return Err("session is busy");
        }
        let token = CancellationToken::new();
        s.cancel = Some(token.clone());
        s.summary.status = SessionStatus::Running;
        Ok((token, std::mem::take(&mut s.history)))
    }

    pub fn finish(&self, id: SessionId, history: Vec<Msg>) {
        if let Some(s) = self.map.lock().unwrap().get_mut(&id) {
            s.history = history;
            s.cancel = None;
        }
    }

    pub fn cancel(&self, id: SessionId) -> bool {
        let map = self.map.lock().unwrap();
        let token = map.get(&id).and_then(|s| s.cancel.as_ref());
        token.inspect(|t| t.cancel()).is_some()
    }

    /// Records an event in memory and in the session's JSONL log.
    pub fn push(&self, id: SessionId, event: &SessionEvent) {
        if let Some(s) = self.map.lock().unwrap().get_mut(&id) {
            s.summary.updated_ms = now_ms();
            match event {
                SessionEvent::Done { .. } => s.summary.status = SessionStatus::Done,
                SessionEvent::Failed { .. } => s.summary.status = SessionStatus::Failed,
                _ => {}
            }
            s.events.push(event.clone());
        }
        self.log(id, json!({"ts_ms": now_ms(), "event": event}));
    }

    /// Appends one JSON line to the session log; failures only go to stderr.
    fn log(&self, id: SessionId, line: serde_json::Value) {
        let path = self.logs.join(format!("{}.jsonl", id.0));
        let r = std::fs::create_dir_all(&self.logs).and_then(|_| {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            writeln!(f, "{line}")
        });
        if let Err(e) = r {
            tracing::warn!(path = %path.display(), error = %e, "writing the session log failed");
        }
    }

    /// Newest first.
    pub fn list(&self) -> Vec<SessionSummary> {
        let mut v: Vec<_> = self
            .map
            .lock()
            .unwrap()
            .values()
            .map(|s| s.summary.clone())
            .collect();
        v.sort_by_key(|s| std::cmp::Reverse(s.updated_ms));
        v
    }

    pub fn get(&self, id: SessionId) -> Option<Vec<SessionEvent>> {
        self.map.lock().unwrap().get(&id).map(|s| s.events.clone())
    }
}
