//! Argument types that accept the loose forms models produce.

use std::borrow::Cow;

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;
use ywza_desktop::WindowId;
use ywza_term::TermId;

/// `w3412106`, `[w3412106]`, `"3412106"` or a JSON number.
pub fn parse_window(v: &Value) -> Option<WindowId> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => {
            let s = s.trim().trim_start_matches('[').trim_end_matches(']');
            s.strip_prefix(['w', 'W']).unwrap_or(s).parse().ok()
        }
        _ => None,
    }
}

pub fn parse_element(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .ok(),
        _ => None,
    }
}

pub fn parse_term(v: &Value) -> Option<TermId> {
    match v {
        Value::Number(n) => n.as_u64().map(|n| TermId(n as u32)),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

macro_rules! arg {
    ($name:ident($inner:ty), $parse:ident, $json_type:literal, $bad:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name(pub $inner);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = Value::deserialize(d)?;
                $parse(&v).map(Self).ok_or_else(|| de::Error::custom($bad))
            }
        }

        impl JsonSchema for $name {
            fn inline_schema() -> bool {
                true
            }

            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({"type": $json_type})
            }
        }
    };
}

arg!(
    WindowArg(WindowId),
    parse_window,
    "string",
    "bad `window` (e.g. \"w3412106\")"
);
arg!(
    ElementArg(u32),
    parse_element,
    "integer",
    "bad `element` (a number)"
);
arg!(
    TermArg(TermId),
    parse_term,
    "string",
    "bad `term` (e.g. \"t2\")"
);

pub fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn window_ids() {
        assert_eq!(parse_window(&json!("w3412106")), Some(3412106));
        assert_eq!(parse_window(&json!("[w12]")), Some(12));
        assert_eq!(parse_window(&json!(" W7 ")), Some(7));
        assert_eq!(parse_window(&json!("3412106")), Some(3412106));
        assert_eq!(parse_window(&json!(3412106)), Some(3412106));
        assert_eq!(parse_window(&json!("t3")), None);
        assert_eq!(parse_window(&json!("w")), None);
        assert_eq!(parse_window(&json!(-1)), None);
        assert_eq!(parse_window(&Value::Null), None);
        assert_eq!(parse_element(&json!("[4]")), Some(4));
        assert_eq!(parse_term(&json!("t2")), Some(TermId(2)));
    }

    #[test]
    fn bad_values_name_the_field() {
        let e = serde_json::from_value::<WindowArg>(json!("nope")).unwrap_err();
        assert!(e.to_string().contains("`window`"), "{e}");
        let e = serde_json::from_value::<ElementArg>(json!(true)).unwrap_err();
        assert!(e.to_string().contains("`element`"), "{e}");
    }
}
