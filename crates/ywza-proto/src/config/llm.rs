use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Llm {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub max_steps: u32,
    pub vision: bool,
}

impl Default for Llm {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "gpt-4.1".into(),
            max_steps: 60,
            vision: true,
        }
    }
}
