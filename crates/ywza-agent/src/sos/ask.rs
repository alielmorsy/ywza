use serde_json::Value;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use ywza_desktop::{self as desk, WindowInfo};
use ywza_proto::{ScreenRect, SessionEvent, SessionId};

use super::{detect, digest, fix_name, uac_window, Blocker, Pause, Paused, NO_CHANGE_STREAK};
use crate::{
    plans::{self, Plan},
    tools::{blocking, parse_window, Space, ToolOutput},
    Agent,
};

impl Agent {
    /// Looks at the result of one local tool call for something the AI must not push through.
    /// `streak` counts results in a row that changed nothing. None: carry on.
    pub(crate) async fn check_blocker(
        &self,
        sid: SessionId,
        name: &str,
        args: &Value,
        out: &ToolOutput,
        streak: &mut u32,
    ) -> Option<Blocker> {
        *streak = if out.changed == Some(false) {
            *streak + 1
        } else {
            0
        };
        let space = Space {
            session: sid,
            sub: None,
        };
        let snap = args
            .get("window")
            .and_then(parse_window)
            .and_then(|w| self.tools.snap(space, w));
        let own_window = || snap.as_ref().map(|s| s.window.clone());
        let mut found = snap
            .as_ref()
            .and_then(|s| detect(s))
            .map(|(k, r)| (k, r, own_window()));
        if found.is_none() {
            let list = blocking(desk::list_windows).await.unwrap_or_default();
            found = uac_window(&list).map(|w| {
                let why = "a Windows permission prompt (UAC) needs the user".to_string();
                ("uac", why, Some(w.clone()))
            });
        }
        if found.is_none() && *streak >= NO_CHANGE_STREAK {
            *streak = 0;
            let why = format!("{NO_CHANGE_STREAK} actions in a row changed nothing");
            found = Some(("stuck", why, own_window()));
        }
        let (kind, reason, window) = found?;
        let fix = fix_name(kind, window.as_ref().map_or("?", |w| w.process.as_str()));
        let sig = format!("{kind}:{}", window.as_ref().map_or(0, |w| w.id));
        let has_fix = plans::store::load(&fix).is_ok();
        tracing::info!(session = %sid.0, tool = name, kind, has_fix, "blocker");
        match (self.sos.sighting(sid, sig), has_fix) {
            (1, true) => Some(Blocker::Hint(format!(
                "blocker {kind} detected; a saved fix exists: run_plan {{\"name\":\"{fix}\"}}"
            ))),
            (1, false) | (2, true) => Some(Blocker::Pause(Pause {
                kind,
                reason,
                window,
                before: snap,
            })),
            _ => None,
        }
    }

    /// Pauses the run until the user resumes (Ok: what to tell the model) or the run is
    /// cancelled (Err).
    pub(crate) async fn ask_for_help(
        &self,
        sid: SessionId,
        pause: Pause,
        cancel: &CancellationToken,
    ) -> Result<String, String> {
        let Pause {
            kind,
            reason,
            window,
            before,
        } = pause;
        let (monitor, rect) = match &window {
            Some(w) => placement(w).await,
            None => (None, None),
        };
        let (tx, rx) = oneshot::channel();
        self.sos.paused.lock().unwrap().insert(
            sid,
            Paused {
                tx,
                snap: before.clone(),
                steps: Vec::new(),
            },
        );
        self.emit(
            sid,
            SessionEvent::Help {
                kind: kind.into(),
                reason,
                window: window.as_ref().map(|w| format!("w{}", w.id)),
                title: window.as_ref().map(|w| w.title.clone()).unwrap_or_default(),
                monitor,
                rect,
            },
        );
        let back = tokio::select! {
            r = rx => r.ok(),
            _ = cancel.cancelled() => None,
        };
        let Some(back) = back else {
            self.sos.paused.lock().unwrap().remove(&sid);
            return Err("cancelled".into());
        };
        let changes = match &before {
            Some(b) => {
                let (w, prev) = (b.window.id, b.clone());
                let fresh = blocking(move || desk::snapshot(w, Some(&prev))).await.ok();
                if let Some(f) = &fresh {
                    let space = Space {
                        session: sid,
                        sub: None,
                    };
                    self.tools.store(space, w, f.clone());
                }
                Some(digest::changes(w, b, fresh.as_ref()))
            }
            None => None,
        };
        let saved = (!back.steps.is_empty())
            .then(|| {
                let process = window.as_ref().map_or("?", |w| w.process.as_str());
                let plan = Plan {
                    name: fix_name(kind, process),
                    description: format!("Taught by the user to clear a {kind} blocker"),
                    steps: back.steps.clone(),
                    ..Default::default()
                };
                plans::store::save(&plan)
                    .map_err(|e| tracing::warn!(error = %format!("{e:#}"), "saving the taught plan"))
                    .ok()
                    .map(|()| plan.name)
            })
            .flatten();
        let mut text = digest::text(
            kind,
            &back.note,
            changes.as_deref(),
            &back.steps,
            saved.as_deref(),
        );
        crate::vault::scrub(&mut text);
        self.emit(sid, SessionEvent::Resumed {
            digest: text.clone(),
        });
        Ok(text)
    }
}

/// The window's monitor and its rect relative to it.
async fn placement(w: &WindowInfo) -> (Option<u32>, Option<ScreenRect>) {
    let Ok(monitors) = blocking(desk::list_monitors).await else {
        return (None, None);
    };
    let Some(m) = monitors.iter().find(|m| m.index == w.monitor) else {
        return (None, None);
    };
    let rect = ScreenRect {
        x: w.rect.x - m.rect.x,
        y: w.rect.y - m.rect.y,
        width: w.rect.w.max(0) as u32,
        height: w.rect.h.max(0) as u32,
    };
    (Some(m.index as u32), Some(rect))
}
