use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use tokio::sync::oneshot;
use ywza_proto::{AgentMsg, SessionId};

struct Waiting {
    session: SessionId,
    tool: String,
    request: AgentMsg,
    tx: oneshot::Sender<bool>,
}

/// Requests waiting for a client's answer. No timeout: the session's loop waits.
#[derive(Default)]
pub struct Pending {
    next: AtomicU64,
    waiting: Mutex<BTreeMap<u64, Waiting>>,
}

impl Pending {
    /// Registers a request; the message goes to the clients, the receiver yields the answer.
    pub fn open(
        &self,
        session: SessionId,
        tool: String,
        summary: String,
        detail: String,
        image: Option<String>,
    ) -> (u64, AgentMsg, oneshot::Receiver<bool>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        let request = AgentMsg::ApprovalRequest {
            request: id,
            session,
            tool: tool.clone(),
            summary,
            detail,
            image,
        };
        let waiting = Waiting {
            session,
            tool,
            request: request.clone(),
            tx,
        };
        self.waiting.lock().unwrap().insert(id, waiting);
        (id, request, rx)
    }

    /// Delivers the answer; returns the request's tool, or None if it is not waiting anymore.
    pub fn answer(&self, request: u64, allow: bool) -> Option<String> {
        let w = self.waiting.lock().unwrap().remove(&request)?;
        let _ = w.tx.send(allow);
        Some(w.tool)
    }

    /// The `ApprovalRequest` of every waiting request (of one session), oldest first.
    pub fn requests(&self, session: Option<SessionId>) -> Vec<AgentMsg> {
        let map = self.waiting.lock().unwrap();
        map.values()
            .filter(|w| session.is_none_or(|s| s == w.session))
            .map(|w| w.request.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_delivered_once() {
        let p = Pending::default();
        let (s1, s2) = (SessionId::new(), SessionId::new());
        let (a, _, rx) = p.open(s1, "run_command".into(), "ls".into(), "{}".into(), None);
        let (b, _, rx_b) = p.open(s2, "term_keys".into(), "x".into(), "{}".into(), None);
        assert_eq!((a, b), (1, 2));
        assert_eq!(p.requests(None).len(), 2);
        assert_eq!(p.requests(Some(s2)).len(), 1);

        assert_eq!(p.answer(a, true).as_deref(), Some("run_command"));
        assert_eq!(p.answer(a, false), None);
        assert_eq!(rx.blocking_recv(), Ok(true));

        drop(rx_b);
        assert_eq!(p.requests(None).len(), 1);
    }
}
