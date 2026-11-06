use std::time::Duration;

use schemars::JsonSchema;
use serde::Deserialize;

use super::{output::truncate, Ctx, Tool, ToolOutput};

pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
}

fn timeout_default() -> u64 {
    120
}

/// Run a one-shot shell command (cmd /C on Windows, sh -c elsewhere) and return its exit code and combined output. Not interactive.
#[derive(Deserialize, JsonSchema)]
pub struct RunCommand {
    command: String,
    cwd: Option<String>,
    #[serde(default = "timeout_default")]
    timeout_s: u64,
}

impl Tool for RunCommand {
    const NAME: &'static str = "run_command";

    async fn run(self, _: &Ctx) -> ToolOutput {
        let mut c = crate::platform::shell(&self.command);
        c.stdin(std::process::Stdio::null()).kill_on_drop(true);
        if let Some(d) = &self.cwd {
            c.current_dir(d);
        }
        let limit = Duration::from_secs(self.timeout_s);
        let out = match tokio::time::timeout(limit, c.output()).await {
            Err(_) => {
                return ToolOutput::err(format!(
                    "timed out after {} s (process killed)",
                    self.timeout_s
                ))
            }
            Ok(Err(e)) => return ToolOutput::err(format!("failed to start: {e}")),
            Ok(Ok(o)) => o,
        };
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        let code = out.status.code().map_or("none".into(), |c| c.to_string());
        ToolOutput {
            ok: out.status.success(),
            text: format!("exit code {code}\n{}", truncate(text)),
            ..Default::default()
        }
    }
}

/// Wait for the given number of milliseconds (max 60000).
#[derive(Deserialize, JsonSchema)]
pub struct Wait {
    ms: u64,
}

impl Tool for Wait {
    const NAME: &'static str = "wait";

    async fn run(self, _: &Ctx) -> ToolOutput {
        let ms = self.ms.min(60_000);
        tokio::time::sleep(Duration::from_millis(ms)).await;
        ToolOutput::ok(format!("waited {ms} ms"))
    }
}

/// Push a short progress message to the user's devices mid-task.
#[derive(Deserialize, JsonSchema)]
pub struct Notify {
    #[allow(dead_code)] // read by the agent loop from the raw args
    text: String,
}

/// The agent loop shows the text to clients; the tool itself only acknowledges.
impl Tool for Notify {
    const NAME: &'static str = "notify";

    async fn run(self, _: &Ctx) -> ToolOutput {
        ToolOutput::ok("sent")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ywza_proto::OcrEngine;

    use crate::tools::Tools;

    #[tokio::test]
    async fn run_command_echo_and_timeout() {
        let t = Tools::default();
        let out = t
            .call("run_command", &json!({"command":"echo hi"}), OcrEngine::Auto)
            .await;
        assert!(out.ok && out.text.contains("hi"), "{out:?}");
        let slow = if cfg!(windows) {
            "ping -n 5 127.0.0.1"
        } else {
            "sleep 5"
        };
        let out = t
            .call(
                "run_command",
                &json!({"command": slow, "timeout_s": 1}),
                OcrEngine::Auto,
            )
            .await;
        assert!(!out.ok && out.text.contains("timed out"));
    }
}
