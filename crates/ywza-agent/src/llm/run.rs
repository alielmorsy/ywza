use std::time::Instant;

use data_encoding::BASE64;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use ywza_proto::{Llm, SessionEvent, SessionId};

use super::{
    client::complete,
    push_tool_result,
    usage::{history_chars, RunUsage},
    Executor, Msg,
};
use crate::{
    sos::Blocker,
    tools::{ToolOutput, SHOW_SCREENSHOT},
    Agent,
};

/// Per-run overrides of the configured model and reasoning effort.
#[derive(Debug, Default, Clone)]
pub struct Opts {
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Every tool runs on a paired device (the executor is a `Remote`): no blocker detection.
    pub remote: bool,
}

/// Runs the loop until the model answers without tool calls (Ok(summary)), the step limit,
/// an error, or cancellation (Err). `history` stays valid for a follow-up prompt either way.
/// Tool calls and the system prompt come from `exec`.
pub async fn run(
    agent: &Agent,
    sid: SessionId,
    history: &mut Vec<Msg>,
    cancel: &CancellationToken,
    opts: &Opts,
    exec: &impl Executor,
) -> Result<String, String> {
    if history.first().is_none_or(|m| m.value["role"] != "system") {
        let prompt = exec.system_prompt().await;
        history.insert(0, Msg::new(json!({"role":"system","content":prompt})));
    }
    let mut llm = agent.config.get().llm;
    if let Some(m) = opts.model.as_ref().filter(|m| !m.is_empty()) {
        llm.model = m.clone();
    }
    if opts.effort.is_some() {
        llm.effort = opts.effort.clone();
    }
    let mut usage = RunUsage::new(&llm);
    let result = steps(agent, sid, history, cancel, &llm, opts, exec, &mut usage).await;
    agent.emit(sid, usage.event());
    result
}

async fn steps(
    agent: &Agent,
    sid: SessionId,
    history: &mut Vec<Msg>,
    cancel: &CancellationToken,
    llm: &Llm,
    opts: &Opts,
    exec: &impl Executor,
    usage: &mut RunUsage,
) -> Result<String, String> {
    let tools = exec.tools();
    let mut streak = 0;
    for step in 0..llm.max_steps {
        let started = Instant::now();
        let (msg, reported) = tokio::select! {
            r = complete(&agent.http, llm, &tools, history) => match r {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!(session = %sid.0, error = %e, "LLM request failed");
                    return Err(e);
                }
            },
            _ = cancel.cancelled() => return Err("cancelled".into()),
        };
        tracing::debug!(
            session = %sid.0,
            step,
            model = %llm.model,
            ms = started.elapsed().as_millis() as u64,
            "LLM request"
        );
        let text = msg["content"].as_str().unwrap_or_default().to_string();
        let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        usage.add_step(
            reported,
            || history_chars(history),
            msg.to_string().len(),
            calls.len() as u32,
        );
        let mut stored = json!({"role":"assistant","content":msg["content"].clone()});
        if !calls.is_empty() {
            stored["tool_calls"] = Value::Array(calls.clone());
        }
        history.push(Msg::new(stored));
        if calls.is_empty() {
            agent.emit(sid, SessionEvent::Assistant { text: text.clone() });
            return Ok(text);
        }
        if !text.trim().is_empty() {
            agent.emit(sid, SessionEvent::Assistant { text });
        }

        let mut images = Vec::new();
        let mut blocker = None;
        for call in &calls {
            let id = call["id"].as_str().unwrap_or_default().to_string();
            let name = call["function"]["name"].as_str().unwrap_or_default();
            // Every tool call must get a result, even after cancel, or the next request is rejected.
            let mut local_args = None;
            let out = if cancel.is_cancelled() {
                ToolOutput::err("cancelled")
            } else {
                let raw = call["function"]["arguments"].as_str().unwrap_or("{}");
                match serde_json::from_str::<Value>(raw) {
                    Err(e) => {
                        tracing::warn!(session = %sid.0, tool = name, error = %e, "tool arguments are not valid JSON");
                        ToolOutput::err(format!("arguments are not valid JSON: {e}"))
                    }
                    Ok(mut args) => {
                        let device = exec.route(&mut args);
                        let device = device.as_deref();
                        agent.emit_tool_call(sid, &id, name, &args, device);
                        let gated = match agent.sessions.approval(sid) {
                            Some(mode) => agent.gate(sid, mode, name, &args, device, cancel).await,
                            None => Ok(()),
                        };
                        let out = match gated {
                            Err(why) => ToolOutput::err(why),
                            Ok(()) => tokio::select! {
                                o = exec.call(sid, device, name, &args) => o,
                                _ = cancel.cancelled() => ToolOutput::err("cancelled"),
                            },
                        };
                        // shortcut: blockers on remote devices are not detected; the remote
                        // agent would need its own SOS, or the check a snapshot of its windows.
                        if device.is_none() && !opts.remote {
                            local_args = Some(args);
                        }
                        out
                    }
                }
            };
            agent.emit_tool_result(sid, &id, name, &out);
            if let Some(args) = &local_args {
                let found = agent.check_blocker(sid, name, args, &out, &mut streak).await;
                blocker = blocker.or(found);
            }
            if let (Some(png), true, false) = (&out.image_png, llm.vision, name == SHOW_SCREENSHOT)
            {
                images.push(json!({"type":"image_url","image_url":{
                    "url": format!("data:image/png;base64,{}", BASE64.encode(png))}}));
            }
            push_tool_result(history, &id, &out.text, out.supersedes_key);
        }
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        if !images.is_empty() {
            let mut content =
                vec![json!({"type":"text","text":"Images from the tool results above."})];
            content.extend(images);
            history.push(Msg::new(json!({"role":"user","content":content})));
        }
        let note = match blocker {
            None => None,
            Some(Blocker::Hint(hint)) => Some(hint),
            Some(Blocker::Pause(p)) => Some(agent.ask_for_help(sid, p, cancel).await?),
        };
        if let Some(note) = note {
            history.push(Msg::new(json!({"role":"user","content":note})));
        }
    }
    tracing::warn!(session = %sid.0, max_steps = llm.max_steps, "step limit reached");
    Err(format!("step limit ({}) reached", llm.max_steps))
}
