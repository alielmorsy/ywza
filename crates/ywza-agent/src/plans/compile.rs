//! Recorded steps to a plan: typed text becomes parameters, each anchored step gets an
//! `expect` guard from its element's name.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde_json::Value;

use super::{record::EXT, store, Param, Plan, PlanStep};

/// The most recently written steps file.
pub fn latest() -> anyhow::Result<PathBuf> {
    let dir = super::logs_dir()?;
    std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(EXT))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
        .ok_or_else(|| anyhow::anyhow!("no recorded steps yet"))
}

pub fn for_session(id: &str) -> anyhow::Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        bail!("bad session id `{id}`");
    }
    let p = super::logs_dir()?.join(format!("{id}.{EXT}"));
    if !p.exists() {
        bail!("no recorded steps for session {id}");
    }
    Ok(p)
}

pub fn compile(steps_file: &Path, name: &str) -> anyhow::Result<Plan> {
    if !store::valid_name(name) {
        bail!("bad plan name `{name}` (letters, digits, - _ . only)");
    }
    let text = std::fs::read_to_string(steps_file)
        .with_context(|| format!("reading {}", steps_file.display()))?;
    let steps = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            serde_json::from_str::<PlanStep>(l).with_context(|| format!("line {}", i + 1))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if steps.is_empty() {
        bail!("{} has no steps", steps_file.display());
    }
    Ok(build(steps, name))
}

fn is_vault(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("{{vault:") && t.ends_with("}}") && t.matches("{{").count() == 1
}

fn slug(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let s = s.split('_').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("_");
    s.chars().take(24).collect()
}

fn unique(params: &[Param], element_name: &str) -> String {
    let base = match slug(element_name) {
        s if s.is_empty() => format!("text{}", params.len() + 1),
        s => s,
    };
    let mut name = base.clone();
    let mut n = 2;
    while params.iter().any(|p| p.name == name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

fn build(steps: Vec<PlanStep>, name: &str) -> Plan {
    let mut params: Vec<Param> = Vec::new();
    let steps = steps
        .into_iter()
        .map(|mut s| {
            let element = s.anchor.as_ref().map_or("", |a| a.name.as_str()).to_string();
            if s.expect.is_none() && !element.is_empty() {
                s.expect = Some(element.clone());
            }
            let text = match (s.tool.as_str(), s.args.get("text")) {
                ("type_text", Some(Value::String(t))) if !t.is_empty() && !is_vault(t) => {
                    Some(t.clone())
                }
                _ => None,
            };
            if let Some(text) = text {
                let pname = unique(&params, &element);
                s.args
                    .insert("text".into(), Value::String(format!("{{{{{pname}}}}}")));
                params.push(Param {
                    name: pname,
                    default: Some(text),
                    ..Default::default()
                });
            }
            s
        })
        .collect();
    Plan {
        name: name.into(),
        params,
        steps,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::SavedAnchor;
    use serde_json::json;

    fn typed(element: &str, text: &str) -> PlanStep {
        PlanStep {
            tool: "type_text".into(),
            args: json!({"text": text}).as_object().unwrap().clone(),
            anchor: Some(SavedAnchor {
                name: element.into(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn typed_text_becomes_params() {
        let plan = build(
            vec![
                typed("Search box", "cats"),
                typed("Search box", "dogs"),
                typed("", "x"),
                typed("Password", "{{vault:pw}}"),
            ],
            "p",
        );
        let names: Vec<&str> = plan.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["search_box", "search_box_2", "text3"]);
        assert_eq!(plan.params[0].default.as_deref(), Some("cats"));
        assert_eq!(plan.steps[1].args["text"], "{{search_box_2}}");
        assert_eq!(plan.steps[3].args["text"], "{{vault:pw}}");
        assert_eq!(plan.steps[0].expect.as_deref(), Some("Search box"));
        assert_eq!(plan.steps[2].expect, None);
    }
}
