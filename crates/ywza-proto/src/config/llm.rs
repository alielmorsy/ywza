use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Llm {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub max_steps: u32,
    pub vision: bool,
    /// Sent as `reasoning_effort` when set (low, medium, high, xhigh, max).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// USD per million input tokens; with `price_out`, the run receipt shows a cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_in: Option<f64>,
    /// USD per million output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_out: Option<f64>,
}

impl Llm {
    pub fn configured(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

impl Default for Llm {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "gpt-4.1".into(),
            max_steps: 60,
            vision: true,
            effort: None,
            price_in: None,
            price_out: None,
        }
    }
}
