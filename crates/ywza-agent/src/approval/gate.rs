use serde_json::Value;
use tokio_util::sync::CancellationToken;
use ywza_proto::{Action, AgentMsg, Approval, ApprovalMode, SessionId};

use super::summary::calls;
use crate::Agent;

pub const DENIED: &str = "denied by the user";
pub const BLOCKED: &str = "blocked by an approval rule";
pub const CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// `tool` and `summary` are of the call that asks (the inner action for a batch).
    Ask {
        tool: String,
        summary: String,
    },
    Deny,
}

/// The strictest answer over the call and, for a batch, each of its actions: any deny denies,
/// else the first ask asks, else allow.
pub fn verdict(approval: &Approval, mode: ApprovalMode, tool: &str, args: &Value) -> Verdict {
    let mut ask = None;
    for (tool, summary) in calls(tool, args) {
        match mode.apply(approval.decide(&tool, &summary)) {
            Action::Deny => return Verdict::Deny,
            Action::Ask => {
                ask.get_or_insert(Verdict::Ask { tool, summary });
            }
            Action::Allow => {}
        }
    }
    ask.unwrap_or(Verdict::Allow)
}

impl Agent {
    /// Lets a tool call of session `sid` through: Ok to run it, Err(reason) to answer the
    /// model with the reason instead. A call that asks waits for `Approve` or for `cancel`.
    pub async fn gate(
        &self,
        sid: SessionId,
        mode: ApprovalMode,
        tool: &str,
        args: &Value,
        cancel: &CancellationToken,
    ) -> Result<(), &'static str> {
        let rules = self.config.get().approval;
        match verdict(&rules, mode, tool, args) {
            Verdict::Allow => Ok(()),
            Verdict::Deny => {
                tracing::info!(session = %sid.0, tool, "tool call blocked by an approval rule");
                Err(BLOCKED)
            }
            Verdict::Ask { tool, summary } => self.ask(sid, tool, summary, args, cancel).await,
        }
    }

    async fn ask(
        &self,
        sid: SessionId,
        tool: String,
        summary: String,
        args: &Value,
        cancel: &CancellationToken,
    ) -> Result<(), &'static str> {
        let mut detail = serde_json::to_string_pretty(args).unwrap_or_default();
        if let Some(Some(device)) = self.sessions.device(sid) {
            detail.push_str(&format!("\n\ndevice: {device}"));
        }
        tracing::info!(session = %sid.0, tool = %tool, summary = %summary, "waiting for approval in the app");
        let (request, msg, answer) = self.approvals.open(sid, tool, summary, detail);
        self.broadcast(msg);
        let r = tokio::select! {
            r = answer => match r {
                Ok(true) => Ok(()),
                Ok(false) => Err(DENIED),
                Err(_) => Err(CANCELLED),
            },
            _ = cancel.cancelled() => {
                self.resolve(request, false);
                Err(CANCELLED)
            }
        };
        tracing::info!(session = %sid.0, request, outcome = r.err().unwrap_or("allowed"), "approval answered");
        r
    }

    /// Answers a waiting request and tells every client. Returns its tool.
    fn resolve(&self, request: u64, allowed: bool) -> Option<String> {
        let tool = self.approvals.answer(request, allowed)?;
        self.broadcast(AgentMsg::ApprovalResolved { request, allowed });
        Some(tool)
    }

    /// A client's `Approve`. `remember`: the caller may change the rules (see `ClientMsg::Approve`).
    pub fn approve(&self, request: u64, allow: bool, remember: bool) -> Option<AgentMsg> {
        let Some(tool) = self.resolve(request, allow) else {
            return Some(AgentMsg::Error {
                message: "no such approval request (already answered?)".into(),
            });
        };
        if allow && remember {
            if let Err(e) = self.config.update(|c| c.approval.remember(&tool)) {
                tracing::error!(error = %e, "saving config failed");
                return Some(AgentMsg::Error {
                    message: format!("saving config: {e}"),
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ywza_proto::Rule;

    use super::*;

    fn rule(tool: &str, pattern: Option<&str>, action: Action) -> Rule {
        Rule {
            tool: tool.into(),
            pattern: pattern.map(Into::into),
            action,
        }
    }

    #[test]
    fn modes_over_the_default_rules() {
        let a = Approval::default();
        let run = json!({"command":"ls"});
        let ask = Verdict::Ask {
            tool: "run_command".into(),
            summary: "ls".into(),
        };
        assert_eq!(verdict(&a, ApprovalMode::Rules, "run_command", &run), ask);
        assert_eq!(
            verdict(&a, ApprovalMode::Auto, "run_command", &run),
            Verdict::Allow
        );
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "click", &json!({})),
            Verdict::Allow
        );
        assert!(matches!(
            verdict(&a, ApprovalMode::Always, "click", &json!({})),
            Verdict::Ask { tool, .. } if tool == "click"
        ));
    }

    #[test]
    fn deny_rules_hold_in_every_mode() {
        let a = Approval {
            rules: vec![rule("run_command", Some("rm *"), Action::Deny)],
        };
        let rm = json!({"command":"rm -rf x"});
        for mode in [
            ApprovalMode::Rules,
            ApprovalMode::Auto,
            ApprovalMode::Always,
        ] {
            assert_eq!(verdict(&a, mode, "run_command", &rm), Verdict::Deny);
        }
    }

    #[test]
    fn a_batch_is_judged_by_its_strictest_action() {
        let a = Approval {
            rules: vec![
                rule("type_text", Some("secret*"), Action::Deny),
                rule("press_keys", None, Action::Ask),
            ],
        };
        let batch = |actions: Value| json!({"window":"w1","actions":actions});
        let ask = batch(json!([{"tool":"wait","ms":1},{"tool":"press_keys","keys":"enter"}]));
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "batch", &ask),
            Verdict::Ask {
                tool: "press_keys".into(),
                summary: "enter".into()
            }
        );
        let deny = batch(json!([
            {"tool":"press_keys","keys":"enter"},
            {"tool":"type_text","text":"secret1"}
        ]));
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "batch", &deny),
            Verdict::Deny
        );
        let fine = batch(json!([{"tool":"wait","ms":1}]));
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "batch", &fine),
            Verdict::Allow
        );
    }
}
