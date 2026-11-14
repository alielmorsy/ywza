//! Replays a plan without the model: each step finds its window and element again, checks its
//! guards, then runs the tool like a normal call.

use serde_json::{json, Map, Value};
use ywza_desktop::{self as desk, Resolve, Snapshot, WindowInfo};

use super::{bind, guard, Plan, PlanStep, WindowRef};
use crate::tools::{blocking, Ctx, ToolOutput};

/// Why a step stopped, and the window's live elements when known.
type Stop = (String, Option<Snapshot>);

fn lost(e: desk::Error) -> Stop {
    (e.to_string(), None)
}

/// Runs the plan from step `from` (1-based). A step that cannot run stops the plan with the
/// reason and the live elements, so the model can patch the step and resume.
pub async fn run(cx: &Ctx, plan: &Plan, params: &Map<String, Value>, from: usize) -> ToolOutput {
    let steps = match bind(plan, params) {
        Ok(s) => s,
        Err(e) => return ToolOutput::err(e),
    };
    let total = steps.len();
    if from == 0 || from > total {
        return ToolOutput::err(format!("`from` must be 1 to {total}"));
    }
    let mut last = String::new();
    for i in from - 1..total {
        match step(cx, &steps, i).await {
            Ok(text) => last = text,
            Err((reason, live)) => return stopped(plan, i, &reason, live),
        }
    }
    let ran = if from == 1 {
        format!("{total} steps")
    } else {
        format!("steps {from} to {total}")
    };
    ToolOutput::ok(format!("plan {}: {ran} ok\n{last}", plan.name))
}

async fn step(cx: &Ctx, steps: &[PlanStep], i: usize) -> Result<String, Stop> {
    let s = &steps[i];
    // A step without its own window acts on the nearest earlier one, except where a window
    // would change the tool's meaning.
    let inherits = !matches!(s.tool.as_str(), "launch" | "wait");
    let wref = s.window.as_ref().or_else(|| {
        steps[..i]
            .iter()
            .rev()
            .find_map(|p| p.window.as_ref())
            .filter(|_| inherits)
    });
    let mut args = s.args.clone();
    let mut live = None;
    let mut window = None;
    if let Some(r) = wref {
        let wins = blocking(desk::list_windows).await.map_err(lost)?;
        let Some(info) = find(&wins, r) else {
            let open: Vec<String> = wins.iter().map(ToString::to_string).collect();
            let reason = format!("no window of {} {:?}; open windows:\n{}", r.process, r.title, open.join("\n"));
            return Err((reason, None));
        };
        let w = info.id;
        window = Some(w);
        if s.tool != "focus_window" && (s.anchor.is_some() || !cx.has_snap(w)) {
            let prev = cx.snap(w);
            let snap = blocking(move || desk::snapshot(w, prev.as_ref()))
                .await
                .map_err(lost)?;
            cx.store(w, snap.clone());
            live = Some(snap);
        }
        args.insert("window".into(), json!(format!("w{w}")));
    }
    match locate(s, live.as_ref()) {
        Ok(Some(id)) => {
            args.insert("element".into(), json!(id));
        }
        Ok(None) => {}
        Err(e) => return Err((e, live)),
    }
    let args = crate::vault::substitute(&s.tool, &Value::Object(args)).map_err(|e| (e, live.clone()))?;
    let mut out = cx.call(&s.tool, args).await;
    crate::vault::scrub(&mut out.text);
    let after = || {
        window
            .and_then(|w| cx.snap(w))
            .filter(|_| !out.text.contains("Fresh element list"))
    };
    if !out.ok {
        return Err((format!("tool failed: {}", out.text), after()));
    }
    if let Some(want) = s.changes.as_deref().filter(|w| !guard::contains(&out.text, w)) {
        return Err((format!("expected changes containing {want:?}, got: {}", out.text), after()));
    }
    Ok(out.text)
}

/// The window of `r`: same process, title containing `r.title` (any case) when one is set and
/// matches (a renamed document must not break the plan), the focused one first.
fn find<'a>(wins: &'a [WindowInfo], r: &WindowRef) -> Option<&'a WindowInfo> {
    let title = r.title.to_lowercase();
    let same = |w: &&WindowInfo| w.process.eq_ignore_ascii_case(&r.process);
    let named = |w: &&WindowInfo| w.title.to_lowercase().contains(&title);
    let pick = |named_only: bool| {
        wins.iter()
            .filter(same)
            .filter(|w| !named_only || named(w))
            .max_by_key(|w| w.focused)
    };
    pick(true).or_else(|| pick(false))
}

/// The id of the step's element in the live list; None for a step without an anchor.
fn locate(s: &PlanStep, snap: Option<&Snapshot>) -> Result<Option<u32>, String> {
    let Some(a) = &s.anchor else {
        return Ok(None);
    };
    let snap = snap.ok_or("the step has no window to find its element in")?;
    let id = match desk::resolve(&a.to_anchor(), snap) {
        Resolve::Found(c) => c[0].id,
        Resolve::Ambiguous(c) => {
            let ids: Vec<String> = c.iter().map(|c| format!("[{}]", c.id)).collect();
            return Err(format!("element is ambiguous: {}", ids.join(" ")));
        }
        Resolve::NotFound => return Err("element not found".into()),
    };
    if let (Some(want), Some(e)) = (&s.expect, snap.get(id)) {
        if !guard::contains(&e.name, want) {
            return Err(format!("guard failed: expected an element named {want:?}, found {}", e.line()));
        }
    }
    Ok(Some(id))
}

fn stopped(plan: &Plan, i: usize, reason: &str, live: Option<Snapshot>) -> ToolOutput {
    let (n, name, s) = (i + 1, &plan.name, &plan.steps[i]);
    let step = serde_json::to_string(s).unwrap_or_default();
    let mut text = format!(
        "plan {name} stopped at step {n}/{} ({}): {reason}\nstep: {step}\n",
        plan.steps.len(),
        s.tool
    );
    if let Some(l) = &live {
        text += &format!("live elements:\n{}\n", l.render());
    }
    text += &format!(
        "fix it with patch_plan {{\"name\":\"{name}\",\"step\":{n},...}} then run_plan {{\"name\":\"{name}\",\"from\":{n}}}"
    );
    crate::vault::scrub(&mut text);
    let out = ToolOutput::err(text);
    match live {
        Some(l) => out.elements(l.window.id),
        None => out,
    }
}
