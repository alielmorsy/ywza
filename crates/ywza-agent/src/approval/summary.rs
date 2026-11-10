use serde_json::Value;

use crate::tools::{parse_batch, parse_window, Step};

fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)?.as_str()
}

/// The actions of a `batch` call; empty for other tools or arguments that do not parse.
fn steps(tool: &str, args: &Value) -> Vec<Step> {
    let list = args.get("actions").and_then(Value::as_array);
    match (tool, list) {
        ("batch", Some(list)) => parse_batch(args.get("window"), list).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// One line that says what the call does: the command, typed text, keys, window id or the
/// actions of a batch; compact JSON for any other tool. Rule patterns match against it.
pub fn summary(tool: &str, args: &Value) -> String {
    let line = match tool {
        "run_command" | "term_start" => text(args, "command").map(String::from),
        "term_write" | "type_text" => text(args, "text").map(String::from),
        "term_keys" | "press_keys" => text(args, "keys").map(String::from),
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
        _ => None,
    };
    let line = line.unwrap_or_else(|| match args {
        Value::Null => String::new(),
        v => v.to_string(),
    });
    line.replace(['\r', '\n'], " ")
}

/// What the rules see for one call: the call itself, then each action of a batch as if it
/// were called alone.
pub fn calls(tool: &str, args: &Value) -> Vec<(String, String)> {
    std::iter::once((tool.to_string(), summary(tool, args)))
        .chain(
            steps(tool, args)
                .into_iter()
                .map(|s| (s.tool.clone(), summary(&s.tool, &s.args))),
        )
        .collect()
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
}
