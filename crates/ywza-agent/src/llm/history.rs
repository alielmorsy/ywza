use serde_json::{json, Value};

pub const SUPERSEDED: &str = "(element list superseded)";

/// One chat message plus the context-hygiene key of a tool result.
#[derive(Debug, Clone)]
pub struct Msg {
    pub value: Value,
    pub key: Option<String>,
}

impl Msg {
    pub fn new(value: Value) -> Self {
        Self { value, key: None }
    }
}

/// Appends a tool result; earlier results with the same `supersedes_key` lose their content.
pub fn push_tool_result(history: &mut Vec<Msg>, call_id: &str, text: &str, key: Option<String>) {
    if let Some(k) = &key {
        for m in history.iter_mut().filter(|m| m.key.as_ref() == Some(k)) {
            m.value["content"] = SUPERSEDED.into();
        }
    }
    history.push(Msg {
        value: json!({"role":"tool","tool_call_id":call_id,"content":text}),
        key,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_element_list_supersedes_older_of_same_window() {
        let mut h = vec![Msg::new(json!({"role":"user","content":"x"}))];
        push_tool_result(&mut h, "a", "list w3 v1", Some("elements:w3".into()));
        push_tool_result(&mut h, "b", "list w4 v1", Some("elements:w4".into()));
        push_tool_result(&mut h, "c", "clicked", None);
        push_tool_result(&mut h, "d", "list w3 v2", Some("elements:w3".into()));
        let contents: Vec<&str> = h
            .iter()
            .map(|m| m.value["content"].as_str().unwrap())
            .collect();
        assert_eq!(
            contents,
            ["x", SUPERSEDED, "list w4 v1", "clicked", "list w3 v2"]
        );
        assert_eq!(h[1].value["tool_call_id"], "a");
    }
}
