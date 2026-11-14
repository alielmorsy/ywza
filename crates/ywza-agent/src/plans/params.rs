//! Plan parameters: `{{name}}` slots in string arguments.

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::{Plan, PlanStep};

fn fill(s: &str, vals: &HashMap<&str, String>) -> String {
    let (mut out, mut rest) = (String::new(), s);
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        match after.find("}}").and_then(|j| Some((j, vals.get(&after[..j])?))) {
            Some((j, v)) => {
                out.push_str(v);
                rest = &after[j + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out + rest
}

fn fill_value(v: &mut Value, vals: &HashMap<&str, String>) {
    match v {
        Value::String(s) => *s = fill(s, vals),
        Value::Array(a) => a.iter_mut().for_each(|x| fill_value(x, vals)),
        Value::Object(m) => m.values_mut().for_each(|x| fill_value(x, vals)),
        _ => {}
    }
}

/// `a (required), b (default "x")`
fn signature(plan: &Plan) -> String {
    if plan.params.is_empty() {
        return "none".into();
    }
    let one = |p: &super::Param| match &p.default {
        Some(d) => format!("{} (default {d:?})", p.name),
        None => format!("{} (required)", p.name),
    };
    plan.params.iter().map(one).collect::<Vec<_>>().join(", ")
}

/// Name, description, params and step count of a plan.
pub fn describe(plan: &Plan) -> String {
    let desc = if plan.description.is_empty() {
        String::new()
    } else {
        format!(": {}", plan.description)
    };
    format!(
        "{}{desc}\n  params: {}; {} steps",
        plan.name,
        signature(plan),
        plan.steps.len()
    )
}

/// The plan's steps with every `{{param}}` filled from `given`, else its default. A missing
/// required param is an error listing the params. `{{vault:..}}` is left for the vault.
pub fn bind(plan: &Plan, given: &Map<String, Value>) -> Result<Vec<PlanStep>, String> {
    let mut vals = HashMap::new();
    for p in &plan.params {
        let v = match given.get(&p.name) {
            Some(Value::String(s)) => s.clone(),
            Some(v) if !v.is_null() => v.to_string(),
            _ => p.default.clone().ok_or_else(|| {
                format!("missing param `{}`; params: {}", p.name, signature(plan))
            })?,
        };
        vals.insert(p.name.as_str(), v);
    }
    let mut steps = plan.steps.clone();
    for s in &mut steps {
        s.args.values_mut().for_each(|v| fill_value(v, &vals));
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plans::Param;
    use serde_json::json;

    #[test]
    fn fills_params_and_leaves_vault() {
        let step = PlanStep {
            tool: "type_text".into(),
            args: json!({"text": "{{q}} {{vault:k}} {{other}}"})
                .as_object()
                .unwrap()
                .clone(),
            ..Default::default()
        };
        let plan = Plan {
            name: "p".into(),
            params: vec![Param {
                name: "q".into(),
                ..Default::default()
            }],
            steps: vec![step],
            ..Default::default()
        };
        let err = bind(&plan, &Map::new()).unwrap_err();
        assert!(err.contains("missing param `q`") && err.contains("q (required)"), "{err}");
        let given = json!({"q": "{{other}}"}).as_object().unwrap().clone();
        let steps = bind(&plan, &given).unwrap();
        assert_eq!(steps[0].args["text"], "{{other}} {{vault:k}} {{other}}");
    }
}
