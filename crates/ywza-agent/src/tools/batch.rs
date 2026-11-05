use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;
use serde_json::Value;
use ywza_desktop::{Snapshot, WindowId};

use super::{
    act::{settle_diff, NO_CHANGES},
    args::parse_window,
    Ctx, Tool, ToolOutput, WindowArg,
};

/// Tools a `batch` may run.
pub const BATCH_TOOLS: &[&str] = &[
    "click",
    "long_press",
    "type_text",
    "press_keys",
    "scroll",
    "wait",
    "focus_window",
];
const BATCH_MAX: usize = 25;

/// One action of a `batch`, with the batch's `window` filled in unless it set its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub tool: String,
    pub args: Value,
}

/// The actions of a `batch` call. Errors name the 1-based action at fault.
pub fn parse_batch(window: Option<&Value>, list: &[Value]) -> Result<Vec<Step>, String> {
    if list.is_empty() || list.len() > BATCH_MAX {
        return Err(format!(
            "`actions` needs 1 to {BATCH_MAX} items, got {}",
            list.len()
        ));
    }
    let window = window.filter(|v| !v.is_null());
    list.iter()
        .enumerate()
        .map(|(i, a)| {
            let n = i + 1;
            let mut a = a
                .as_object()
                .cloned()
                .ok_or(format!("action {n}: not an object"))?;
            let tool = a
                .remove("tool")
                .and_then(|t| t.as_str().map(String::from))
                .ok_or(format!("action {n}: missing `tool`"))?;
            if !BATCH_TOOLS.contains(&tool.as_str()) {
                return Err(format!(
                    "action {n}: `{tool}` is not allowed in a batch (allowed: {})",
                    BATCH_TOOLS.join(", ")
                ));
            }
            if let (true, Some(w)) = (a.get("window").is_none_or(Value::is_null), window) {
                a.insert("window".into(), w.clone());
            }
            Ok(Step {
                tool,
                args: Value::Object(a),
            })
        })
        .collect()
}

/// Outcome of one batch action; `error` set = it failed.
pub struct StepResult {
    pub tool: String,
    pub error: Option<String>,
}

/// `1 click ok`, `2 type_text ok`, `3 press_keys FAILED: <error>`, plus a note for the actions
/// never run (the batch stops at the first failure).
pub fn format_steps(results: &[StepResult], total: usize) -> String {
    let mut lines: Vec<String> = results
        .iter()
        .enumerate()
        .map(|(i, r)| match &r.error {
            None => format!("{} {} ok", i + 1, r.tool),
            Some(e) => format!("{} {} FAILED: {e}", i + 1, r.tool),
        })
        .collect();
    if results.len() < total {
        lines.push(format!("({} more actions not run)", total - results.len()));
    }
    lines.join("\n")
}

fn actions_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "minItems": 1,
        "description": format!(
            "Each item is {{\"tool\": ...}} plus that tool's arguments. Tools: {}.",
            BATCH_TOOLS.join(", ")
        ),
        "items": {
            "type": "object",
            "properties": {"tool": {"type": "string", "enum": BATCH_TOOLS}},
            "required": ["tool"],
            "additionalProperties": true
        }
    })
}

/// Run several UI actions in order in one call, stopping at the first failure. Element ids come from the window's latest get_elements, from before the batch. Inner actions inherit `window` unless they set their own. Returns one line per action, then the element changes since the batch started (or the fresh list if an id went stale).
#[derive(Deserialize, JsonSchema)]
pub struct Batch {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: Option<WindowArg>,
    #[schemars(schema_with = "actions_schema")]
    actions: Vec<Value>,
}

impl Tool for Batch {
    const NAME: &'static str = "batch";

    /// Runs the steps through the registry (each element action validates, re-snapshots and
    /// keeps ids stable, but does not settle or diff) and stops at the first failure. Then
    /// settles each touched window once and reports its changes since the batch started.
    async fn run(self, cx: &Ctx) -> ToolOutput {
        let window = self.window.map(|w| Value::String(format!("w{}", w.0)));
        let steps = match parse_batch(window.as_ref(), &self.actions) {
            Ok(s) => s,
            Err(e) => return ToolOutput::err(e),
        };
        let inner = cx.quieter();
        let mut results = Vec::new();
        let mut baselines: Vec<(WindowId, Snapshot)> = Vec::new();
        let mut failure: Option<ToolOutput> = None;
        for step in &steps {
            if let Some(w) = parse_window(&step.args["window"]) {
                if let (true, Some(s)) = (baselines.iter().all(|(b, _)| *b != w), cx.snap(w)) {
                    baselines.push((w, s));
                }
            }
            let out = inner.call(&step.tool, step.args.clone()).await;
            results.push(StepResult {
                tool: step.tool.clone(),
                error: (!out.ok).then(|| out.text.clone()),
            });
            if !out.ok {
                failure = Some(out);
                break;
            }
        }
        let mut text = format_steps(&results, steps.len());
        // A stale error already carries the fresh element list; no diff on top of it.
        let stale = failure
            .as_ref()
            .is_some_and(|o| o.text.starts_with("stale:"));
        let supersedes_key = failure.as_ref().and_then(|o| o.supersedes_key.clone());
        if !stale {
            let many = baselines.len() > 1;
            for (w, base) in baselines {
                let Some(d) = settle_diff(cx, w, &base).await else {
                    continue;
                };
                let head = if many {
                    format!("changes in w{w}")
                } else {
                    "changes".into()
                };
                text += &if d == NO_CHANGES {
                    format!("\n{head}: none visible")
                } else {
                    format!("\n{head}:\n{}", d.trim_end())
                };
            }
        }
        ToolOutput {
            text,
            ok: failure.is_none(),
            supersedes_key,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ywza_proto::OcrEngine;

    use super::*;
    use crate::tools::Tools;

    fn steps(args: Value) -> Result<Vec<Step>, String> {
        let list = args["actions"].as_array().cloned().unwrap_or_default();
        parse_batch(args.get("window"), &list)
    }

    #[test]
    fn batch_actions_inherit_the_window() {
        let args = json!({"window":"w7","actions":[
            {"tool":"click","element":12},
            {"tool":"type_text","element":30,"text":"hi","submit":true},
            {"tool":"press_keys","keys":"enter","window":"w9"},
            {"tool":"scroll","window":null,"dy":3},
            {"tool":"wait","ms":500}]});
        let steps = steps(args).unwrap();
        let tools: Vec<&str> = steps.iter().map(|s| s.tool.as_str()).collect();
        assert_eq!(
            tools,
            ["click", "type_text", "press_keys", "scroll", "wait"]
        );
        let windows: Vec<&Value> = steps.iter().map(|s| &s.args["window"]).collect();
        assert_eq!(windows, ["w7", "w7", "w9", "w7", "w7"]);
        assert_eq!(
            steps[1].args,
            json!({"window":"w7","element":30,"text":"hi","submit":true})
        );
        // No batch window: inner actions keep what they have.
        let steps = self::steps(json!({"actions":[{"tool":"wait","ms":1}]})).unwrap();
        assert!(steps[0].args.get("window").is_none());
    }

    #[test]
    fn batch_rejects_bad_input() {
        let err = |v: Value| steps(v).unwrap_err();
        assert!(err(json!({"actions":[]})).contains("1 to"));
        assert!(err(json!({"actions":[{"tool":"launch"}]}))
            .contains("action 1: `launch` is not allowed"));
        assert!(err(json!({"actions":[{"tool":"wait"},{"tool":"batch"}]})).contains("action 2"));
        assert!(err(json!({"actions":[{"element":1}]})).contains("missing `tool`"));
        assert!(err(json!({"actions":[5]})).contains("not an object"));
        let many = vec![json!({"tool":"wait"}); BATCH_MAX + 1];
        assert!(err(json!({"actions":many})).contains("1 to"));
    }

    #[test]
    fn batch_report_stops_at_the_first_failure() {
        let r = |tool: &str, error: Option<&str>| StepResult {
            tool: tool.into(),
            error: error.map(String::from),
        };
        let all = [r("click", None), r("type_text", None)];
        assert_eq!(format_steps(&all, 2), "1 click ok\n2 type_text ok");
        let stopped = [
            r("click", None),
            r("type_text", None),
            r("press_keys", Some("input was blocked")),
        ];
        assert_eq!(
            format_steps(&stopped, 5),
            "1 click ok\n2 type_text ok\n3 press_keys FAILED: input was blocked\n(2 more actions not run)"
        );
    }

    #[tokio::test]
    async fn batch_of_waits_reports_each_action() {
        let t = Tools::default();
        let args = json!({"window":"w1","actions":[{"tool":"wait","ms":1},{"tool":"wait","ms":2}]});
        let out = t.call("batch", &args, OcrEngine::Auto).await;
        assert!(out.ok, "{out:?}");
        assert_eq!(out.text, "1 wait ok\n2 wait ok");
    }

    #[tokio::test]
    async fn batch_stops_when_an_element_action_has_no_element_list() {
        // Nothing touches the desktop: there is no stored snapshot for w1.
        let t = Tools::default();
        let args = json!({"window":"w1","actions":[
            {"tool":"wait","ms":1},
            {"tool":"click","element":3},
            {"tool":"wait","ms":1}]});
        let out = t.call("batch", &args, OcrEngine::Auto).await;
        assert!(!out.ok);
        assert!(
            out.text
                .starts_with("1 wait ok\n2 click FAILED: no element list for w1"),
            "{}",
            out.text
        );
        assert!(
            out.text.ends_with("(1 more actions not run)"),
            "{}",
            out.text
        );
    }
}
