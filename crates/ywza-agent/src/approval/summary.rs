use serde_json::Value;

use crate::{
    plans,
    tools::{parse_batch, parse_window, Step},
};

fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)?.as_str()
}

/// The actions of a `batch` call, or the steps `run_plan` would run (params filled in); empty
/// for other tools or arguments that do not parse.
pub(super) fn steps(tool: &str, args: &Value) -> Vec<Step> {
    let list = args.get("actions").and_then(Value::as_array);
    match (tool, list) {
        ("batch", Some(list)) => parse_batch(args.get("window"), list).unwrap_or_default(),
        ("run_plan", _) => plan_steps(args),
        _ => Vec::new(),
    }
}

fn plan_steps(args: &Value) -> Vec<Step> {
    let Some(plan) = text(args, "name").and_then(|n| plans::store::load(n).ok()) else {
        return Vec::new();
    };
    let params = args.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
    let from = args.get("from").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
    plans::bind(&plan, &params)
        .unwrap_or_default()
        .into_iter()
        .skip(from - 1)
        .map(|s| Step {
            tool: s.tool,
            args: Value::Object(s.args),
        })
        .collect()
}

/// One line that says what the call does: the command, typed text, keys, window id or the
/// actions of a batch; compact JSON for any other tool. Rule patterns match against it.
pub fn summary(tool: &str, args: &Value) -> String {
    let line = match tool {
        "run_command" | "term_start" | "term_run" => text(args, "command").map(String::from),
        "term_write" | "type_text" => text(args, "text").map(String::from),
        "term_keys" | "press_keys" => text(args, "keys").map(String::from),
        "copy" => text(args, "src")
            .zip(text(args, "dst"))
            .map(|(src, dst)| format!("{src} -> {dst}")),
        "close_window" | "focus_window" | "move_window" => args
            .get("window")
            .and_then(parse_window)
            .map(|w| format!("w{w}")),
        "launch" => text(args, "program").map(|program| {
            let rest = args["args"].as_array().into_iter().flatten();
            let rest = rest.filter_map(Value::as_str);
            std::iter::once(program)
                .chain(rest)
                .collect::<Vec<_>>()
                .join(" ")
        }),
        "batch" => Some(steps(tool, args)).filter(|s| !s.is_empty()).map(|s| {
            s.iter()
                .map(|s| format!("{}: {}", s.tool, summary(&s.tool, &s.args)))
                .collect::<Vec<_>>()
                .join("; ")
        }),
        "run_plan" => text(args, "name").map(|name| {
            let params = args.get("params").filter(|p| !p.is_null());
            format!("{name} {}", params.map_or("{}".into(), Value::to_string))
        }),
        _ => None,
    };
    let line = line.unwrap_or_else(|| match args {
        Value::Null => String::new(),
        v => v.to_string(),
    });
    line.replace(['\r', '\n'], " ")
}

/// What the rules see for one call: the call itself, then each action of a batch as if it
/// were called alone. A `fanout` adds the tool it runs, so that tool's risk and rules apply.
pub fn calls(tool: &str, args: &Value) -> Vec<(String, String)> {
    let mut all: Vec<_> = std::iter::once((tool.to_string(), summary(tool, args)))
        .chain(
            steps(tool, args)
                .into_iter()
                .map(|s| (s.tool.clone(), summary(&s.tool, &s.args))),
        )
        .collect();
    if let (true, Some(inner)) = (tool == "fanout", text(args, "tool")) {
        let none = Value::Null;
        all.extend(calls(inner, args.get("args").unwrap_or(&none)));
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summaries_pick_the_relevant_argument() {
        assert_eq!(
            summary("run_command", &json!({"command":"ls\n-la","cwd":"/"})),
            "ls -la"
        );
        assert_eq!(
            summary(
                "type_text",
                &json!({"window":"w1","text":"hi","submit":true})
            ),
            "hi"
        );
        assert_eq!(
            summary("term_keys", &json!({"term":"t1","keys":"ctrl+c"})),
            "ctrl+c"
        );
        assert_eq!(summary("close_window", &json!({"window":"[w42]"})), "w42");
        assert_eq!(
            summary("launch", &json!({"program":"git","args":["push","-f"]})),
            "git push -f"
        );
        assert_eq!(summary("wait", &json!({"ms":5})), r#"{"ms":5}"#);
        assert_eq!(summary("list_windows", &Value::Null), "");
        assert_eq!(
            summary("run_plan", &json!({"name":"p","params":{"a":"b"}})),
            r#"p {"a":"b"}"#
        );
        assert_eq!(summary("run_plan", &json!({"name":"p"})), "p {}");
        assert!(steps("run_plan", &json!({"name":"p"})).is_empty());
    }

    #[test]
    fn batch_lists_its_actions_and_exposes_them_to_rules() {
        let args = json!({"window":"w1","actions":[
            {"tool":"wait","ms":5},{"tool":"type_text","text":"x","element":1}]});
        let s = summary("batch", &args);
        assert!(
            s.starts_with("wait: ") && s.ends_with("; type_text: x"),
            "{s}"
        );
        let all = calls("batch", &args);
        assert_eq!(all.len(), 3);
        assert_eq!((all[2].0.as_str(), all[2].1.as_str()), ("type_text", "x"));
        assert_eq!(calls("click", &json!({})).len(), 1);
    }

    #[test]
    fn fanout_exposes_the_tool_it_runs_and_copy_reads_as_an_arrow() {
        let args = json!({"tool":"run_command","args":{"command":"rm x"},"devices":["vm"]});
        let all = calls("fanout", &args);
        assert_eq!(all.len(), 2);
        assert_eq!((all[1].0.as_str(), all[1].1.as_str()), ("run_command", "rm x"));
        assert_eq!(calls("fanout", &json!({"tool":"find_app","devices":[]})).len(), 2);
        assert_eq!(
            summary("copy", &json!({"src":"vm:/a","dst":"E:\\b"})),
            "vm:/a -> E:\\b"
        );
    }
}
