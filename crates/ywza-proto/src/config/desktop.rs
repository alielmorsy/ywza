use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Desktop {
    pub agent_virtual_desktop: bool,
    pub idle_before_input_ms: u32,
    /// Show a mark on the target of each action before it acts.
    pub lens: bool,
    /// How long a mark is shown before the action runs.
    pub lens_dwell_ms: u32,
}

impl Default for Desktop {
    fn default() -> Self {
        Self {
            agent_virtual_desktop: false,
            idle_before_input_ms: 1000,
            lens: true,
            lens_dwell_ms: 120,
        }
    }
}
