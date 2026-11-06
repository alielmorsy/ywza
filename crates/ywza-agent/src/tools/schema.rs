//! Tool schemas from `JsonSchema`, tidied for LLM function calling.

use schemars::generate::SchemaSettings;
use schemars::JsonSchema;
use serde_json::{json, Value};

/// The struct doc comment and the JSON Schema of the arguments.
pub fn describe<T: JsonSchema>() -> (String, Value) {
    let schema = SchemaSettings::draft07()
        .with(|s| s.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<T>();
    let mut v = serde_json::to_value(&schema).unwrap_or_default();
    let doc = ["description", "title"]
        .iter()
        .find_map(|k| v[k].as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    tidy(&mut v);
    if let Some(root) = v.as_object_mut() {
        for k in ["$schema", "title", "description"] {
            root.remove(k);
        }
        root.entry("properties").or_insert_with(|| json!({}));
        root.entry("required").or_insert_with(|| json!([]));
    }
    (doc, v)
}

/// Optional fields become plain types (no `null` alternative) and numeric formats go.
fn tidy(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if let Some(Value::Array(t)) = m.get_mut("type") {
                t.retain(|x| x != "null");
                if t.len() == 1 {
                    let one = t.remove(0);
                    m.insert("type".into(), one);
                }
            }
            if let Some(Value::Array(e)) = m.get_mut("enum") {
                e.retain(|x| !x.is_null());
            }
            let numeric = matches!(m.get("format"),
                Some(Value::String(f)) if f.starts_with("int") || f.starts_with("uint"));
            if numeric {
                m.remove("format");
            }
            m.values_mut().for_each(tidy);
        }
        Value::Array(a) => a.iter_mut().for_each(tidy),
        _ => {}
    }
}

/// Error text for arguments that did not deserialize, naming the offending field when a
/// provided value has the wrong JSON kind.
pub fn bad_args(name: &str, err: &serde_json::Error, schema: &Value, args: &Value) -> String {
    match blame(schema, args) {
        Some(field) => format!("bad arguments for `{name}`: {field} ({err})"),
        None => format!("bad arguments for `{name}`: {err}"),
    }
}

fn blame(schema: &Value, args: &Value) -> Option<String> {
    let props = schema.get("properties")?.as_object()?;
    let given = args.as_object()?;
    props.iter().find_map(|(k, p)| {
        let v = given.get(k).filter(|v| !v.is_null())?;
        let want = p.get("type")?.as_str()?;
        let ok = match want {
            "integer" => v.is_i64() || v.is_u64(),
            "boolean" => v.is_boolean(),
            "array" => v.is_array(),
            "object" => v.is_object(),
            // Window, element and terminal ids are strings or numbers.
            "string" => !(v.is_boolean() || v.is_array() || v.is_object()),
            _ => true,
        };
        (!ok).then(|| format!("`{k}` should be {want}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// Does a thing.
    #[derive(Deserialize, JsonSchema, Debug)]
    #[allow(dead_code)]
    struct Sample {
        a: u32,
        b: Option<String>,
        #[serde(default)]
        c: bool,
    }

    #[test]
    fn schema_is_plain() {
        let (doc, s) = describe::<Sample>();
        assert_eq!(doc, "Does a thing.");
        assert_eq!(s["type"], "object");
        assert_eq!(s["required"], json!(["a"]));
        assert_eq!(s["properties"]["b"]["type"], "string");
        assert_eq!(s["properties"]["a"]["type"], "integer");
        assert!(s["properties"]["a"].get("format").is_none());
        assert_eq!(s["properties"]["c"]["default"], false);
        assert!(s.get("$schema").is_none() && s.get("title").is_none());
    }

    #[test]
    fn wrong_kind_is_blamed_on_its_field() {
        let (_, s) = describe::<Sample>();
        let args = json!({"a": "three", "b": "x"});
        let e = serde_json::from_value::<Sample>(args.clone()).unwrap_err();
        let msg = bad_args("sample", &e, &s, &args);
        assert!(msg.contains("`a` should be integer"), "{msg}");
        let args = json!({});
        let e = serde_json::from_value::<Sample>(args.clone()).unwrap_err();
        assert!(bad_args("sample", &e, &s, &args).contains("missing field `a`"));
    }
}
