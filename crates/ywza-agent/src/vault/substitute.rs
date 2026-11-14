use serde_json::Value;

use super::store::Entries;

pub const OPEN: &str = "{{vault:";

/// Replaces `{{vault:key}}` in `text`. Substituted values are not scanned again.
fn replace(text: &str, entries: &Entries) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(OPEN) {
        out.push_str(&rest[..i]);
        let after = &rest[i + OPEN.len()..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[i..]);
            return Ok(out);
        };
        let key = &after[..end];
        let value = entries
            .get(key)
            .ok_or_else(|| format!("unknown vault key `{key}`"))?;
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn text_field(args: &mut Value, entries: &Entries) -> Result<(), String> {
    if let Some(Value::String(t)) = args.get_mut("text") {
        *t = replace(t, entries)?;
    }
    Ok(())
}

/// Copy of `args` with placeholders replaced, only where a tool types text: `type_text` and
/// `term_write` `text`, and the `type_text` steps of a `batch`.
pub fn substitute_args(name: &str, args: &Value, entries: &Entries) -> Result<Value, String> {
    let mut args = args.clone();
    match name {
        "type_text" | "term_write" => text_field(&mut args, entries)?,
        "batch" => {
            if let Some(steps) = args.get_mut("actions").and_then(Value::as_array_mut) {
                for step in steps.iter_mut().filter(|s| s["tool"] == "type_text") {
                    text_field(step, entries)?;
                }
            }
        }
        _ => {}
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn vault() -> Entries {
        Entries::from([("gh".into(), "s3cret".into()), ("a".into(), "{{vault:gh}}".into())])
    }

    #[test]
    fn type_text_and_term_write_get_the_value() {
        let args = json!({"element":3,"text":"user:{{vault:gh}}!"});
        for tool in ["type_text", "term_write"] {
            let out = substitute_args(tool, &args, &vault()).unwrap();
            assert_eq!(out, json!({"element":3,"text":"user:s3cret!"}));
        }
    }

    #[test]
    fn values_are_not_expanded_again() {
        let out = substitute_args("type_text", &json!({"text":"{{vault:a}}"}), &vault()).unwrap();
        assert_eq!(out["text"], "{{vault:gh}}");
    }

    #[test]
    fn batch_only_touches_type_text_steps() {
        let args = json!({"actions":[
            {"tool":"type_text","text":"{{vault:gh}}"},
            {"tool":"press_keys","keys":"{{vault:gh}}"},
            {"tool":"click","element":1}]});
        let out = substitute_args("batch", &args, &vault()).unwrap();
        assert_eq!(out["actions"][0]["text"], "s3cret");
        assert_eq!(out["actions"][1]["keys"], "{{vault:gh}}");
    }

    #[test]
    fn unknown_key_is_an_error_even_inside_a_batch() {
        let e = substitute_args("type_text", &json!({"text":"{{vault:nope}}"}), &vault());
        assert_eq!(e.unwrap_err(), "unknown vault key `nope`");
        let batch = json!({"actions":[{"tool":"type_text","text":"{{vault:nope}}"}]});
        assert!(substitute_args("batch", &batch, &vault()).is_err());
    }

    #[test]
    fn other_tools_fields_and_plain_text_are_left_alone() {
        let args = json!({"command":"echo {{vault:gh}}","text":"{{vault:nope"});
        assert_eq!(substitute_args("run_command", &args, &vault()).unwrap(), args);
        let plain = json!({"text":"hello {{ not vault }}"});
        assert_eq!(substitute_args("type_text", &plain, &vault()).unwrap(), plain);
        let open = json!({"text":"a {{vault:gh"});
        assert_eq!(substitute_args("type_text", &open, &vault()).unwrap(), open);
    }
}
