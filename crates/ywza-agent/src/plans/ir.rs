//! The plan format: what the model writes, what a recorded run compiles to, what replay runs.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::anchor::SavedAnchor;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    pub steps: Vec<PlanStep>,
}

/// A value the caller passes in; steps use it as `{{name}}` inside string arguments.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Used when the caller leaves it out; None: required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Which window a step acts on. Window ids die with the window, so it is found again by process
/// and, when set, a title substring.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowRef {
    pub process: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    /// A batch tool (`BATCH_TOOLS`) or `launch`.
    pub tool: String,
    /// The tool's arguments without `window` and `element`; strings may hold `{{param}}`.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub args: Map<String, Value>,
    /// None: the previous step's window (or no window, e.g. `launch`, `wait`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowRef>,
    /// The element acted on, for element actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<SavedAnchor>,
    /// Precondition: the resolved element's name contains this (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<String>,
    /// Expected effect: the step's rendered changes contain this (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<String>,
}
