use serde_json::{json, Value};
use ywza_proto::Llm;

use super::Msg;
use crate::tools::Tools;

/// One OpenAI-compatible `/chat/completions` request; returns the assistant message.
pub async fn complete(
    http: &reqwest::Client,
    llm: &Llm,
    tools: &Tools,
    history: &[Msg],
) -> Result<Value, String> {
    let tools: Vec<Value> = tools
        .specs()
        .into_iter()
        .filter(|t| llm.vision || t.name != "screenshot")
        .map(|t| {
            json!({"type":"function","function":{
            "name":t.name,"description":t.description,"parameters":t.parameters}})
        })
        .collect();
    let body = json!({
        "model": llm.model,
        "messages": history.iter().map(|m| &m.value).collect::<Vec<_>>(),
        "tools": tools,
    });
    let url = format!("{}/chat/completions", llm.base_url.trim_end_matches('/'));
    let mut req = http.post(url).json(&body);
    if !llm.api_key.is_empty() {
        req = req.bearer_auth(&llm.api_key);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("LLM response: {e}"))?;
    if !status.is_success() {
        let snippet: String = text.chars().take(500).collect();
        return Err(format!("LLM returned {status}: {snippet}"));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("LLM response: {e}"))?;
    match &v["choices"][0]["message"] {
        Value::Null => Err(format!(
            "LLM response has no message: {}",
            text.chars().take(500).collect::<String>()
        )),
        m => Ok(m.clone()),
    }
}
