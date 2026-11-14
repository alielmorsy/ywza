//! Smart lens: follows a window and sends its tappable elements, keeping them current.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use ywza_desktop::{self as desk, WindowId};
use ywza_proto::{AgentMsg, ElementInfo, ScreenRect, SessionId};

use super::chips;
use crate::{
    inspect::{origin, rel_rect},
    tools::Space,
    Agent,
};

/// Without UI events (or while following the focus) the window is checked this often.
const POLL: Duration = Duration::from_millis(700);
/// A snapshot is taken at least this often even when no event arrived.
const FORCE: Duration = Duration::from_secs(2);
/// Event bursts (blinking carets, animations) never snapshot faster than this.
const MIN_GAP: Duration = Duration::from_millis(150);
const MAX_FAILS: u32 = 3;

/// A client's running lens; dropping it stops the loop and forgets its element ids.
pub struct Lens {
    stop: Arc<AtomicBool>,
    agent: Arc<Agent>,
    space: Space,
}

impl Drop for Lens {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.agent.tools.drop_session(self.space.session);
    }
}

impl Lens {
    /// `window` None follows the focused window on `monitor` (any monitor when None). The lens
    /// has its own element space, so its ids never collide with other callers'.
    pub fn start(
        agent: Arc<Agent>,
        window: Option<WindowId>,
        monitor: Option<u32>,
        to_client: UnboundedSender<AgentMsg>,
    ) -> Self {
        let space = Space {
            session: SessionId::new(),
            sub: None,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (a, s) = (agent.clone(), stop.clone());
        std::thread::spawn(move || {
            run(&a, space, &s, window, monitor, &to_client);
            a.tools.drop_session(space.session);
        });
        Self { stop, agent, space }
    }

    pub fn space(&self) -> Space {
        self.space
    }
}

/// What the client holds of the followed window.
struct Sent {
    window: WindowId,
    meta: (String, ScreenRect, u32),
    chips: HashMap<u32, ElementInfo>,
}

fn run(
    agent: &Agent,
    space: Space,
    stop: &AtomicBool,
    fixed: Option<WindowId>,
    monitor: Option<u32>,
    tx: &UnboundedSender<AgentMsg>,
) {
    let (mut win, mut sent, mut count) = (fixed, None::<Sent>, 0);
    let (mut last, mut fails) = (None::<Instant>, 0);
    tracing::info!(?fixed, ?monitor, "lens started");
    while !stop.load(Ordering::Relaxed) {
        if fixed.is_none() {
            win = pick(monitor, win);
        }
        let Some(w) = win else {
            std::thread::sleep(POLL);
            continue;
        };
        if sent.as_ref().map(|s| s.window) != Some(w) {
            count = desk::wait_activity(w, u64::MAX, Duration::ZERO).count;
        } else {
            let wait = if fixed.is_some() { FORCE } else { POLL };
            let a = desk::wait_activity(w, count, wait);
            if !a.live {
                std::thread::sleep(POLL);
            }
            count = a.count;
            let forced = last.is_none_or(|t| t.elapsed() >= FORCE);
            if !(a.changed || !a.live || forced) {
                continue;
            }
        }
        if let Some(rest) = last.and_then(|t| MIN_GAP.checked_sub(t.elapsed())) {
            std::thread::sleep(rest);
        }
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let prev = agent.tools.snap(space, w);
        let snap = match desk::snapshot(w, prev.as_ref()) {
            Ok(s) => s,
            Err(e) => {
                fails += 1;
                if fixed.is_some() && fails >= MAX_FAILS {
                    if let Some(s) = sent.take() {
                        let _ = tx.send(message(w, s.meta, true, Vec::new(), Vec::new()));
                    }
                    let _ = tx.send(AgentMsg::Error {
                        message: format!("lens: w{w}: {e}"),
                    });
                    return;
                }
                std::thread::sleep(POLL);
                continue;
            }
        };
        fails = 0;
        last = Some(Instant::now());
        if stop.load(Ordering::Relaxed) {
            return;
        }
        agent.tools.store(space, w, snap.clone());
        let mons = desk::list_monitors().unwrap_or_default();
        let org = origin(&mons, snap.window.monitor);
        let now = chips::project(&snap.elements, org);
        let meta = (
            snap.window.title.clone(),
            rel_rect(org, snap.window.rect),
            snap.window.monitor as u32,
        );
        let held = sent.take().filter(|s| s.window == w);
        let (up, removed) = match &held {
            Some(s) => chips::changes(&s.chips, &now),
            None => (now.clone(), Vec::new()),
        };
        let same = held.as_ref().is_some_and(|s| s.meta == meta);
        if !(same && up.is_empty() && removed.is_empty()) {
            let msg = message(w, meta.clone(), held.is_none(), up, removed);
            if tx.send(msg).is_err() {
                return;
            }
        }
        sent = Some(Sent {
            window: w,
            meta,
            chips: now.into_iter().map(|c| (c.id, c)).collect(),
        });
    }
}

fn message(
    w: WindowId,
    (title, rect, monitor): (String, ScreenRect, u32),
    full: bool,
    elements: Vec<ElementInfo>,
    removed: Vec<u32>,
) -> AgentMsg {
    AgentMsg::Lens {
        window: format!("w{w}"),
        title,
        monitor,
        rect,
        full,
        elements,
        removed,
    }
}

/// The window to show: the focused one on `monitor`, else the current one while it stays
/// visible there, else the first visible one there.
fn pick(monitor: Option<u32>, cur: Option<WindowId>) -> Option<WindowId> {
    let wins = desk::list_windows().ok()?;
    let ok = |w: &&desk::WindowInfo| {
        !w.minimized && !w.hidden && monitor.is_none_or(|m| w.monitor == m as usize)
    };
    wins.iter()
        .filter(ok)
        .find(|w| w.focused)
        .or_else(|| wins.iter().filter(ok).find(|w| Some(w.id) == cur))
        .or_else(|| wins.iter().find(ok))
        .map(|w| w.id)
}

/// The tool call a `LensAct` stands for.
fn tool_call(
    window: &str,
    element: u32,
    text: Option<String>,
    submit: bool,
) -> (&'static str, Value) {
    match text {
        None => ("click", json!({"window": window, "element": element})),
        Some(text) => (
            "type_text",
            json!({"window": window, "element": element, "text": text, "submit": submit}),
        ),
    }
}

/// Runs a `LensAct` in the lens's space, like a tool call (vault, background actions).
pub async fn act(
    agent: &Agent,
    space: Space,
    call: u64,
    window: String,
    element: u32,
    text: Option<String>,
    submit: bool,
) -> AgentMsg {
    let (tool, args) = tool_call(&window, element, text, submit);
    let out = agent.run_tool(space, tool, &args).await;
    AgentMsg::ToolResult {
        call,
        ok: out.ok,
        text: out.text,
        png_base64: out.image_png.map(|p| data_encoding::BASE64.encode(&p)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acts_click_or_type() {
        let (tool, args) = tool_call("w3", 7, None, true);
        assert_eq!(tool, "click");
        assert_eq!(args, json!({"window":"w3","element":7}));
        let (tool, args) = tool_call("w3", 8, Some("hi".into()), true);
        assert_eq!(tool, "type_text");
        assert_eq!(
            args,
            json!({"window":"w3","element":8,"text":"hi","submit":true})
        );
    }
}
