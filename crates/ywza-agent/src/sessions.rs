//! Session store: in memory, plus `<config_dir>/logs/<session>.jsonl` activity logs.

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use ywza_proto::{ApprovalMode, SessionEvent, SessionId, SessionStatus, SessionSummary};

use crate::llm::Msg;

struct Session {
    summary: SessionSummary,
    events: Vec<SessionEvent>,
    history: Vec<Msg>,
    /// Some while the loop runs.
    cancel: Option<CancellationToken>,
    /// Set by prompts that carry a mode (the desktop app); None: tools run ungated.
    approval: Option<ApprovalMode>,
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

fn title_of(prompt: &str) -> String {
    let mut title: String = prompt.chars().take(60).collect();
    if title.len() < prompt.len() {
        title.push('…');
    }
    title
}

fn append(logs: &Path, id: SessionId, line: &Value) {
    let path = logs.join(format!("{}.jsonl", id.0));
    let r = std::fs::create_dir_all(logs).and_then(|_| {
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

/// The conversation gist for a follow-up: user and assistant texts, tool calls dropped.
fn history_from(events: &[SessionEvent]) -> Vec<Msg> {
    events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::User { text } => Some(("user", text)),
            SessionEvent::Assistant { text } => Some(("assistant", text)),
            _ => None,
        })
        .map(|(role, text)| Msg::new(json!({"role": role, "content": text})))
        .collect()
}

/// Reads every `<id>.jsonl`; unreadable files and lines are skipped. A log that does not end
/// in done/failed was cut off by a restart: it is closed with a failed event.
fn load(logs: &Path) -> HashMap<SessionId, Session> {
    let mut map = HashMap::new();
    let Ok(dir) = std::fs::read_dir(logs) else {
        return map;
    };
    for path in dir.flatten().map(|e| e.path()) {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|_| path.extension().is_some_and(|e| e == "jsonl"))
            .and_then(|s| serde_json::from_value::<SessionId>(Value::String(s.into())).ok());
        let (Some(id), Ok(bytes)) = (id, std::fs::read(&path)) else {
            continue;
        };
        let mut summary = None;
        let mut events = Vec::new();
        let mut last_ts = 0;
        for line in String::from_utf8_lossy(&bytes).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            last_ts = v["ts_ms"].as_u64().unwrap_or(last_ts);
            if let Some(s) = v.get("session") {
                summary = serde_json::from_value::<SessionSummary>(s.clone())
                    .ok()
                    .or(summary);
            } else if let Some(e) = v.get("event") {
                events.extend(serde_json::from_value::<SessionEvent>(e.clone()));
            }
        }
        let first_user = events.iter().find_map(|e| match e {
            SessionEvent::User { text } => Some(title_of(text)),
            _ => None,
        });
        let Some(mut summary) = summary.or_else(|| {
            first_user.map(|title| SessionSummary {
                id,
                title,
                status: SessionStatus::Running,
                updated_ms: 0,
                device: None,
                devices: Vec::new(),
            })
        }) else {
            continue;
        };
        summary.updated_ms = last_ts.max(summary.updated_ms);
        if !matches!(
            events.last(),
            Some(SessionEvent::Done { .. } | SessionEvent::Failed { .. })
        ) {
            let failed = SessionEvent::Failed {
                error: "interrupted: the agent restarted".into(),
            };
            summary.updated_ms = now_ms();
            append(
                logs,
                id,
                &json!({"ts_ms": summary.updated_ms, "event": failed}),
            );
            events.push(failed);
        }
        summary.status = match events.last() {
            Some(SessionEvent::Done { .. }) => SessionStatus::Done,
            _ => SessionStatus::Failed,
        };
        let session = Session {
            summary,
            events,
            history: Vec::new(),
            cancel: None,
            approval: None,
        };
        map.insert(id, session);
    }
    map
}

impl Sessions {
    /// Loads the sessions found in `logs`.
    pub fn new(logs: PathBuf) -> Self {
        let map = Mutex::new(load(&logs));
        Self { logs, map }
    }

    pub fn create(&self, prompt: &str) -> SessionId {
        let id = SessionId::new();
        self.create_with(id, prompt, None, &[]);
        id
    }

    /// Creates the session `id` unless it exists (clients may pick their own ids).
    pub fn create_with(
        &self,
        id: SessionId,
        prompt: &str,
        device: Option<&str>,
        devices: &[String],
    ) {
        let mut map = self.map.lock().unwrap();
        if map.contains_key(&id) {
            return;
        }
        let summary = SessionSummary {
            id,
            title: title_of(prompt),
            status: SessionStatus::Running,
            updated_ms: now_ms(),
            device: device.map(String::from),
            devices: devices.to_vec(),
        };
        let line = json!({"ts_ms": summary.updated_ms, "session": summary});
        map.insert(
            id,
            Session {
                summary,
                events: Vec::new(),
                history: Vec::new(),
                cancel: None,
                approval: None,
            },
        );
        drop(map);
        self.log(id, line);
    }

    /// The paired device the session operates and the extra devices it may use; None: no such session.
    pub fn target(&self, id: SessionId) -> Option<(Option<String>, Vec<String>)> {
        let map = self.map.lock().unwrap();
        let s = &map.get(&id)?.summary;
        Some((s.device.clone(), s.devices.clone()))
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
        if s.history.is_empty() {
            s.history = history_from(&s.events);
        }
        Ok((token, std::mem::take(&mut s.history)))
    }

    pub fn finish(&self, id: SessionId, history: Vec<Msg>) {
        if let Some(s) = self.map.lock().unwrap().get_mut(&id) {
            s.history = history;
            s.cancel = None;
        }
    }

    pub fn approval(&self, id: SessionId) -> Option<ApprovalMode> {
        let map = self.map.lock().unwrap();
        map.get(&id).and_then(|s| s.approval)
    }

    pub fn set_approval(&self, id: SessionId, mode: ApprovalMode) {
        if let Some(s) = self.map.lock().unwrap().get_mut(&id) {
            s.approval = Some(mode);
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
    fn log(&self, id: SessionId, line: Value) {
        append(&self.logs, id, &line);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, id: SessionId, lines: &[Value]) {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(dir.join(format!("{}.jsonl", id.0)), text).unwrap();
    }

    fn ev(ts: u64, e: SessionEvent) -> Value {
        json!({"ts_ms": ts, "event": e})
    }

    fn call() -> SessionEvent {
        SessionEvent::ToolCall {
            id: "c".into(),
            name: "wait".into(),
            args: json!({}),
            device: None,
        }
    }

    #[test]
    fn logs_are_loaded_and_interrupted_ones_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (done, cut) = (SessionId::new(), SessionId::new());
        let summary = SessionSummary {
            id: done,
            title: "hello".into(),
            status: SessionStatus::Running,
            updated_ms: 10,
            device: Some("vm".into()),
            devices: Vec::new(),
        };
        write(
            dir.path(),
            done,
            &[
                json!({"ts_ms": 10, "session": summary}),
                ev(
                    11,
                    SessionEvent::User {
                        text: "hello".into(),
                    },
                ),
                ev(12, call()),
                ev(13, SessionEvent::Assistant { text: "hi".into() }),
                ev(
                    14,
                    SessionEvent::Done {
                        summary: "hi".into(),
                    },
                ),
            ],
        );
        // An old log: no session line, cut off after a tool call, with a garbage line.
        write(
            dir.path(),
            cut,
            &[
                ev(
                    20,
                    SessionEvent::User {
                        text: "old chat".into(),
                    },
                ),
                json!("not an event"),
                ev(25, call()),
            ],
        );
        std::fs::write(dir.path().join("notes.jsonl"), "x\n").unwrap();

        let s = Sessions::new(dir.path().to_path_buf());
        let all = s.list();
        assert_eq!(all.len(), 2);

        let a = all.iter().find(|x| x.id == done).unwrap();
        assert_eq!(a.title, "hello");
        assert_eq!(a.status, SessionStatus::Done);
        assert_eq!(a.device.as_deref(), Some("vm"));
        assert_eq!(a.updated_ms, 14);
        assert_eq!(s.get(done).unwrap().len(), 4);

        let b = all.iter().find(|x| x.id == cut).unwrap();
        assert_eq!((b.title.as_str(), b.device.clone()), ("old chat", None));
        assert_eq!(b.status, SessionStatus::Failed);
        let events = s.get(cut).unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events.last(),
            Some(SessionEvent::Failed { error }) if error == "interrupted: the agent restarted"
        ));
        let log = std::fs::read_to_string(dir.path().join(format!("{}.jsonl", cut.0))).unwrap();
        assert!(log.lines().last().unwrap().contains("interrupted"), "{log}");

        // A follow-up keeps the user and assistant texts, without tool calls.
        let (_, history) = s.begin(done).unwrap();
        let roles: Vec<_> = history
            .iter()
            .map(|m| m.value["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["user", "assistant"]);
        assert_eq!(history[1].value["content"], "hi");

        // Loading again does not close the log twice.
        let again = Sessions::new(dir.path().to_path_buf());
        assert_eq!(again.get(cut).unwrap().len(), 3);
    }

    #[test]
    fn new_sessions_write_their_summary_first() {
        let dir = tempfile::tempdir().unwrap();
        let s = Sessions::new(dir.path().to_path_buf());
        let id = SessionId::new();
        s.create_with(id, "run it", Some("vm"), &["pc".into()]);
        s.push(
            id,
            &SessionEvent::User {
                text: "run it".into(),
            },
        );
        let log = std::fs::read_to_string(dir.path().join(format!("{}.jsonl", id.0))).unwrap();
        let first: Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
        assert_eq!(first["session"]["title"], "run it");
        assert_eq!(first["session"]["device"], "vm");
        assert_eq!(first["session"]["devices"][0], "pc");
        assert_eq!(s.target(id), Some((Some("vm".into()), vec!["pc".into()])));
        assert_eq!(s.approval(id), None);
        s.set_approval(id, ApprovalMode::Rules);
        assert_eq!(s.approval(id), Some(ApprovalMode::Rules));
    }
}
