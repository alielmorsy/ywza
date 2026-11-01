use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Desktop {
    pub agent_virtual_desktop: bool,
    pub idle_before_input_ms: u32,
}

impl Default for Desktop {
    fn default() -> Self {
        Self {
            agent_virtual_desktop: false,
            idle_before_input_ms: 1000,
        }
    }
}
