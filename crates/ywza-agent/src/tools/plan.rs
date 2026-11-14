//! Plan tools: replay what was recorded or written once, without planning each step again.

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{Ctx, Tool, ToolOutput, BATCH_TOOLS};
use crate::plans::{self, store};

fn params_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "description": "Values for the plan's params by name, as strings. Omitted params use their default.",
        "additionalProperties": {"type": "string"}
    })
}

fn step_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "tool": {"type": "string", "enum": BATCH_TOOLS.iter().copied().chain(["launch"]).collect::<Vec<_>>()},
            "args": {
                "type": "object",
                "description": "The tool's arguments. `window` (\"w123\") and `element` (id) as in a normal call; they are stored as a process/title and a durable anchor. String values may hold {{param}}."
            },
            "expect": {"type": "string", "description": "The element's name must contain this (any case); defaults to the element's own name."},
            "changes": {"type": "string", "description": "The step's reported changes must contain this (any case)."}
        },
        "required": ["tool"]
    })
}

fn plan_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Letters, digits, - _ . only."},
            "description": {"type": "string"},
            "params": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "description": {"type": "string"},
                        "default": {"type": "string", "description": "Omit to make the param required."}
                    },
                    "required": ["name"]
                }
            },
            "steps": {"type": "array", "minItems": 1, "items": step_schema()}
        },
        "required": ["name", "steps"]
    })
}

fn with_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "description": "The replacement step.",
        "properties": step_schema()["properties"],
        "required": ["tool"]
    })
}

/// List saved plans: name, description, params (default or required) and step count.
#[derive(Deserialize, JsonSchema)]
pub struct ListPlans {}

impl Tool for ListPlans {
    const NAME: &'static str = "list_plans";

    async fn run(self, _: &Ctx) -> ToolOutput {
        let list = store::list();
        if list.is_empty() {
            return ToolOutput::ok("no plans saved");
        }
        ToolOutput::ok(list.iter().map(plans::describe).collect::<Vec<_>>().join("\n"))
    }
}

/// Replay a saved plan without planning each step: every step finds its window and element again from what was recorded and checks its guards. On a mismatch it stops and returns the step and the window's live elements: fix the step with patch_plan, then run_plan again with `from` set to that step. Element ids in the result are valid for later actions.
#[derive(Deserialize, JsonSchema)]
pub struct RunPlan {
    name: String,
    #[serde(default)]
    #[schemars(schema_with = "params_schema")]
    params: Map<String, Value>,
    /// 1-based step to start from, to resume after a patch.
    from: Option<usize>,
}

impl Tool for RunPlan {
    const NAME: &'static str = "run_plan";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        match store::load(&self.name) {
            Ok(plan) => plans::replay::run(cx, &plan, &self.params, self.from.unwrap_or(1)).await,
            Err(e) => ToolOutput::err(format!("{e:#}")),
        }
    }
}

/// Save a plan for run_plan (replaces one of the same name). Steps are batch tools or launch. Give `window` and `element` in a step's args exactly as in a normal call, from your latest get_elements: they are stored as a durable anchor and the element's name becomes the step's guard. Declare a param in `params` and write {{name}} in string args to make a value variable; {{vault:key}} stays a vault secret.
#[derive(Deserialize, JsonSchema)]
pub struct SavePlan {
    #[schemars(schema_with = "plan_schema")]
    plan: Value,
}

impl Tool for SavePlan {
    const NAME: &'static str = "save_plan";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        let plan = match plans::plan_from(cx, &self.plan) {
            Ok(p) => p,
            Err(e) => return ToolOutput::err(e),
        };
        match store::save(&plan) {
            Ok(()) => ToolOutput::ok(format!("saved plan {}: {} steps", plan.name, plan.steps.len())),
            Err(e) => ToolOutput::err(format!("{e:#}")),
        }
    }
}

/// Change one step of a saved plan: replace it with `with` (a step as in save_plan) or delete it with `remove`. Use it to fix the step run_plan stopped at.
#[derive(Deserialize, JsonSchema)]
pub struct PatchPlan {
    name: String,
    /// 1-based.
    step: usize,
    #[serde(default)]
    #[schemars(schema_with = "with_schema")]
    with: Option<Value>,
    #[serde(default)]
    remove: bool,
}

impl Tool for PatchPlan {
    const NAME: &'static str = "patch_plan";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        let mut plan = match store::load(&self.name) {
            Ok(p) => p,
            Err(e) => return ToolOutput::err(format!("{e:#}")),
        };
        let n = self.step;
        if n == 0 || n > plan.steps.len() {
            return ToolOutput::err(format!("`step` must be 1 to {}", plan.steps.len()));
        }
        let done = match (&self.with, self.remove) {
            (Some(v), false) => match plans::step_from(cx, v) {
                Ok(s) => {
                    plan.steps[n - 1] = s;
                    format!("patched step {n}")
                }
                Err(e) => return ToolOutput::err(e),
            },
            (None, true) if plan.steps.len() > 1 => {
                plan.steps.remove(n - 1);
                format!("removed step {n}, {} left", plan.steps.len())
            }
            (None, true) => return ToolOutput::err("a plan needs at least one step; delete it instead"),
            _ => return ToolOutput::err("give exactly one of `with` and `remove`"),
        };
        match store::save(&plan) {
            Ok(()) => ToolOutput::ok(format!("{done} of plan {}", plan.name)),
            Err(e) => ToolOutput::err(format!("{e:#}")),
        }
    }
}
