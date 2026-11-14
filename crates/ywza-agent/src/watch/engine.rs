//! The watcher loop: one tick a second, one window list shared by every watcher, window
//! reads shared through [`Shots`]. Nothing here uses the AI until an action asks for it.

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{Local, NaiveDateTime};
use tokio::time::MissedTickBehavior;
use ywza_desktop::{self as desk, Snapshot, WindowInfo};
use ywza_proto::{AgentMsg, ClientMsg, OcrEngine};
use ywza_term::TermId;

use super::{
    check::{
        element_state, find_named, term_finished, term_line, text_finder, title_match, TermNow,
    },
    model::{Action, Condition, Watcher},
    schedule,
    shots::Shots,
    store::{self, Store},
    trigger::{gate, Edge, Gate},
};
use crate::{
    tools::{blocking, Space},
    Agent,
};

const TICK: Duration = Duration::from_secs(1);
const TERM_LINES: usize = 200;

/// Runtime state of one enabled watcher; lost when it is turned off or the agent restarts.
#[derive(Default)]
struct Run {
    edge: Edge,
    /// window_closes: a matching window has been seen.
    seen: bool,
    /// element_changes: the element's last known state.
    state: Option<String>,
    term: Option<TermNow>,
    next: Option<NaiveDateTime>,
    recent: VecDeque<u64>,
}

enum Truth {
    /// Could not be read this tick; nothing changes.
    Unknown,
    Miss,
    Hit(String),
}

pub fn spawn(agent: Arc<Agent>) {
    tokio::spawn(run(agent));
}

async fn run(agent: Arc<Agent>) {
    let Ok(store) = store::get() else { return };
    let mut runs: HashMap<String, Run> = HashMap::new();
    let mut shots = Shots::default();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let list: Vec<Watcher> = store.list().into_iter().filter(|w| w.enabled).collect();
        runs.retain(|id, _| list.iter().any(|w| w.id == *id));
        if list.is_empty() {
            shots.sweep();
            continue;
        }
        let wins = if list.iter().any(|w| w.when.needs_windows()) {
            blocking(desk::list_windows).await.ok()
        } else {
            None
        };
        for w in &list {
            let run = runs.entry(w.id.clone()).or_default();
            let hit = match eval(&agent, &w.when, run, wins.as_deref(), &mut shots).await {
                Truth::Unknown => continue,
                Truth::Miss => None,
                Truth::Hit(d) => Some(d),
            };
            let rose = if w.when.is_event() {
                hit.is_some()
            } else {
                run.edge.rise(hit.is_some())
            };
            if let (true, Some(detail)) = (rose, hit) {
                fire(&agent, store, w, run, detail);
            }
        }
        shots.sweep();
    }
}

fn find_win<'a>(wins: &'a [WindowInfo], title: &str) -> Option<&'a WindowInfo> {
    let want = title.to_lowercase();
    wins.iter().find(|w| title_match(w, &want))
}

/// The first window titled like `title` and its element list.
async fn tree(
    wins: &[WindowInfo],
    title: &str,
    shots: &mut Shots,
) -> Result<(WindowInfo, Arc<Snapshot>), Truth> {
    let w = find_win(wins, title).ok_or(Truth::Miss)?;
    let s = shots.tree(w.id).await.ok_or(Truth::Unknown)?;
    Ok((w.clone(), s))
}

fn ocr_engine(c: OcrEngine) -> ywza_ocr::Engine {
    match c {
        OcrEngine::Auto => ywza_ocr::Engine::Auto,
        OcrEngine::Windows => ywza_ocr::Engine::Windows,
        OcrEngine::Paddle => ywza_ocr::Engine::Paddle,
    }
}

async fn eval(
    agent: &Agent,
    cond: &Condition,
    run: &mut Run,
    wins: Option<&[WindowInfo]>,
    shots: &mut Shots,
) -> Truth {
    if cond.needs_windows() && wins.is_none() {
        return Truth::Unknown;
    }
    let wins = wins.unwrap_or_default();
    match cond {
        Condition::WindowAppears { title } => match find_win(wins, title) {
            Some(w) => Truth::Hit(format!("window appeared: {w}")),
            None => Truth::Miss,
        },
        Condition::WindowCloses { title } => match find_win(wins, title) {
            Some(_) => {
                run.seen = true;
                Truth::Miss
            }
            None if run.seen => Truth::Hit(format!("window closed: {title:?}")),
            None => Truth::Miss,
        },
        Condition::ElementAppears { title, text } => match tree(wins, title, shots).await {
            Ok((w, s)) => match text_finder(text)(&s) {
                Some(l) => Truth::Hit(format!("{text:?} appeared in {:?}: {l}", w.title)),
                None => Truth::Miss,
            },
            Err(t) => t,
        },
        Condition::ElementChanges { title, text } => {
            let (w, s) = match tree(wins, title, shots).await {
                Ok(x) => x,
                Err(t) => return t,
            };
            let Some(e) = find_named(&s, &text.to_lowercase()) else {
                return Truth::Miss;
            };
            let now = Some(element_state(e));
            let changed = run.state.is_some() && run.state != now;
            run.state = now;
            if changed {
                Truth::Hit(format!("element changed in {:?}: {}", w.title, e.line()))
            } else {
                Truth::Miss
            }
        }
        Condition::Text { title, text, ocr } => {
            let (w, s) = match tree(wins, title, shots).await {
                Ok(x) => x,
                Err(t) => return t,
            };
            if let Some(l) = text_finder(text)(&s) {
                return Truth::Hit(format!("{text:?} in {:?}: {l}", w.title));
            }
            if !ocr {
                return Truth::Miss;
            }
            let engine = ocr_engine(agent.config.get().ocr.engine);
            let want = desk::ocr_norm(text);
            match shots.ocr(w.id, engine).await {
                Some(lines) if lines.iter().any(|l| desk::ocr_matches(&desk::ocr_norm(l), &want)) => {
                    Truth::Hit(format!("{text:?} on screen in {:?}", w.title))
                }
                Some(_) => Truth::Miss,
                None => Truth::Unknown,
            }
        }
        Condition::TermOutput { term, text } => {
            let (terms, id) = (agent.tools.terms(), TermId(*term));
            let out = blocking(move || terms.read(id, TERM_LINES)).await;
            match out.as_ref().ok().and_then(|o| term_line(o, &text.to_lowercase())) {
                Some(l) => Truth::Hit(format!("{id} output: {}", l.trim())),
                None => Truth::Miss,
            }
        }
        Condition::TermExit { term, code } => {
            let (terms, id) = (agent.tools.terms(), TermId(*term));
            let now = blocking(move || {
                let t = terms.owned(id).ok()?;
                let info = t.shell_info();
                Some(TermNow {
                    busy: info.as_ref().is_some_and(|i| i.busy),
                    exit: info.and_then(|i| i.exit),
                    proc_exit: t.exit_status().map(|c| c as i32),
                })
            })
            .await;
            let Some(now) = now else {
                return Truth::Miss;
            };
            let Some(prev) = run.term.replace(now.clone()) else {
                return Truth::Miss;
            };
            match term_finished(&prev, &now, *code) {
                Some(c) => Truth::Hit(format!("{id} finished with exit code {c}")),
                None => Truth::Miss,
            }
        }
        Condition::Schedule { every_min, at } => {
            let at = at.as_deref().and_then(schedule::parse_at);
            let now = Local::now().naive_local();
            if schedule::due(&mut run.next, *every_min, at, now) {
                Truth::Hit(format!("scheduled: {}", cond.describe()))
            } else {
                Truth::Miss
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn announce(agent: &Agent, w: &Watcher, text: String) {
    agent.broadcast(AgentMsg::WatchFired {
        watcher: w.id.clone(),
        name: w.name.clone(),
        text,
    });
}

fn fire(agent: &Arc<Agent>, store: &Store, w: &Watcher, run: &mut Run, detail: String) {
    let now = now_ms();
    match gate(now, w.last_fired_ms, &mut run.recent, w.min_gap_s, w.max_per_hour) {
        Gate::Skip => {}
        Gate::Disable => {
            if let Err(e) = store.set_enabled(&w.id, false) {
                tracing::warn!(error = %e, id = %w.id, "turning a watcher off failed");
            }
            announce(
                agent,
                w,
                format!(
                    "turned off: it fired more than {} times in an hour",
                    w.max_per_hour
                ),
            );
        }
        Gate::Fire => {
            store.record_fire(&w.id, now);
            tracing::info!(id = %w.id, name = %w.name, %detail, "watcher fired");
            match &w.then {
                Action::Notify { text } => {
                    announce(agent, w, text.clone().unwrap_or(detail));
                }
                Action::Prompt { text, device } => {
                    let msg = ClientMsg::Prompt {
                        session: None,
                        text: format!("{text}\n\n(Started by watcher {:?}: {detail})", w.name),
                        device: device.clone(),
                        devices: Vec::new(),
                        model: None,
                        effort: None,
                        approval: None,
                    };
                    let errs: Vec<String> = agent
                        .handle(msg, true)
                        .into_iter()
                        .filter_map(|m| match m {
                            AgentMsg::Error { message } => Some(message),
                            _ => None,
                        })
                        .collect();
                    let note = match errs.first() {
                        Some(e) => format!("{detail} (starting the prompt failed: {e})"),
                        None => format!("{detail} (prompt started)"),
                    };
                    announce(agent, w, note);
                }
                Action::Plan { name } => {
                    let (agent, w, name) = (agent.clone(), w.clone(), name.clone());
                    tokio::spawn(async move {
                        let space = Space {
                            session: ywza_proto::SessionId::new(),
                            sub: None,
                        };
                        let args = serde_json::json!({ "name": name });
                        let out = agent.run_tool(space, "run_plan", &args).await;
                        agent.tools.drop_session(space.session);
                        let state = if out.ok { "plan ran" } else { "plan stopped" };
                        let first = out.text.lines().next().unwrap_or_default();
                        announce(&agent, &w, format!("{detail} ({state}: {first})"));
                    });
                }
            }
        }
    }
}
