//! Steps and plans as the model writes them: `window` ("w123") and `element` (an id) in a
//! step's args become a window reference and an anchor, taken from the stored element list.

use serde_json::{json, Value};

use super::{element_id, record::recordable, store, Plan, PlanStep, WindowRef};
use crate::tools::{parse_window, Ctx};

/// A step with `tool`, optional `args`, `expect` and `changes`. `window` and `element` may sit
/// in `args` or beside it. A named element becomes the step's `expect` unless one is given.
pub fn step_from(cx: &Ctx, v: &Value) -> Result<PlanStep, String> {
    let mut m = v.as_object().cloned().ok_or("a step is an object")?;
    let tool = m.get("tool").and_then(Value::as_str).unwrap_or_default();
    if !recordable(tool) {
        return Err(format!("`{tool}` is not allowed in a plan"));
    }
    let mut args = match m.remove("args") {
        Some(Value::Object(a)) => a,
        None | Some(Value::Null) => Default::default(),
        _ => return Err("`args` must be an object".into()),
    };
    let wid = args.remove("window").filter(|v| !v.is_null());
    let beside = m.get("window").is_some_and(|w| !w.is_object());
    let wid = if beside { m.remove("window") } else { wid };
    let el = args
        .remove("element")
        .or_else(|| m.remove("element"))
        .filter(|v| !v.is_null());
    let mut step: PlanStep =
        serde_json::from_value(Value::Object(m)).map_err(|e| e.to_string())?;
    step.args = args;
    let Some(wid) = wid else {
        return match el {
            Some(_) => Err("`element` needs a `window`".into()),
            None => Ok(step),
        };
    };
    let w = parse_window(&wid).ok_or("bad `window` (e.g. \"w3412106\")")?;
    let snap = cx
        .snap(w)
        .ok_or(format!("no element list for w{w}; call get_elements first"))?;
    step.window = Some(WindowRef {
        process: snap.window.process.clone(),
        title: snap.window.title.clone(),
    });
    if let Some(el) = el {
        let id = element_id(&el).ok_or("bad `element` (a number)")?;
        let e = snap.get(id).ok_or(format!("no element [{id}] in w{w}"))?;
        step.anchor = Some((&e.anchor).into());
        if step.expect.is_none() && !e.name.is_empty() {
            step.expect = Some(e.name.clone());
        }
    }
    Ok(step)
}

/// A plan whose steps go through `step_from`; valid name, at least one step.
pub fn plan_from(cx: &Ctx, v: &Value) -> Result<Plan, String> {
    let mut m = v.as_object().cloned().ok_or("`plan` must be an object")?;
    let Some(Value::Array(raw)) = m.remove("steps") else {
        return Err("`plan.steps` must be an array".into());
    };
    let steps = raw
        .iter()
        .enumerate()
        .map(|(i, s)| step_from(cx, s).map_err(|e| format!("step {}: {e}", i + 1)))
        .collect::<Result<Vec<_>, _>>()?;
    m.insert("steps".into(), json!([]));
    let mut plan: Plan = serde_json::from_value(Value::Object(m)).map_err(|e| e.to_string())?;
    if !store::valid_name(&plan.name) {
        return Err(format!(
            "bad plan name `{}` (letters, digits, - _ . only)",
            plan.name
        ));
    }
    if steps.is_empty() {
        return Err("a plan needs at least one step".into());
    }
    plan.steps = steps;
    Ok(plan)
}
