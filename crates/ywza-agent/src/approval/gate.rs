use data_encoding::BASE64;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use ywza_proto::{Action, AgentMsg, Approval, ApprovalMode, SessionId};

use super::{
    resolve::{resolve, shot, Resolved},
    summary::calls,
};
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
/// else the first ask asks, else allow. A call no rule matches asks when `risky(tool)`.
/// `resolved[i]` is what call `i` of `calls` targets (see `resolve`): a rule pattern matches it
/// or the plain summary, so `type_text secret*` and `*button*` both work. It is also what asks.
pub fn verdict(
    approval: &Approval,
    mode: ApprovalMode,
    tool: &str,
    args: &Value,
    resolved: &[Option<String>],
    risky: impl Fn(&str) -> bool,
) -> Verdict {
    let mut ask = None;
    for (i, (tool, summary)) in calls(tool, args).into_iter().enumerate() {
        let line = resolved.get(i).and_then(|l| l.as_deref());
        let seen: Vec<&str> = line.into_iter().chain([summary.as_str()]).collect();
        let action = approval.decide_any(&tool, &seen).unwrap_or(if risky(&tool) {
            Action::Ask
        } else {
            Action::Allow
        });
        match mode.apply(action) {
            Action::Deny => return Verdict::Deny,
            Action::Ask => {
                let summary = line.map_or(summary, String::from);
                ask.get_or_insert(Verdict::Ask { tool, summary });
            }
            Action::Allow => {}
        }
    }
    ask.unwrap_or(Verdict::Allow)
}

/// Whether any call could ask or deny, so that resolving what it targets is worth the time.
fn worth_resolving(
    approval: &Approval,
    mode: ApprovalMode,
    tool: &str,
    args: &Value,
    risky: &impl Fn(&str) -> bool,
) -> bool {
    mode == ApprovalMode::Always
        || calls(tool, args)
            .iter()
            .any(|(t, _)| risky(t) || approval.rules.iter().any(|r| r.tool == *t))
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
        device: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<(), &'static str> {
        let rules = self.config.get().approval;
        // `fanout` is judged by the tool it runs (see `calls`); `copy` is unknown to the
        // registry, so it asks.
        let risky = |t: &str| t != "fanout" && self.tools.risky(t);
        let found = if device.is_none() && worth_resolving(&rules, mode, tool, args, &risky) {
            let space = crate::tools::Space {
                session: sid,
                sub: None,
            };
            resolve(&self.tools, space, tool, args).await
        } else {
            Vec::new()
        };
        let lines: Vec<_> = found
            .iter()
            .map(|r| r.as_ref().map(|r| r.line.clone()))
            .collect();
        match verdict(&rules, mode, tool, args, &lines, risky) {
            Verdict::Allow => Ok(()),
            Verdict::Deny => {
                tracing::info!(session = %sid.0, tool, "tool call blocked by an approval rule");
                Err(BLOCKED)
            }
            Verdict::Ask { tool, summary } => {
                self.ask(sid, (tool, summary), args, device, &found, cancel)
                    .await
            }
        }
    }

    async fn ask(
        &self,
        sid: SessionId,
        (tool, summary): (String, String),
        args: &Value,
        device: Option<&str>,
        found: &[Option<Resolved>],
        cancel: &CancellationToken,
    ) -> Result<(), &'static str> {
        let mut head: Vec<String> = found.iter().flatten().map(|r| r.line.clone()).collect();
        let device = device
            .map(String::from)
            .or_else(|| self.sessions.target(sid)?.0);
        if let Some(device) = device {
            head.push(format!("device: {device}"));
        }
        let raw = serde_json::to_string_pretty(args).unwrap_or_default();
        let detail = if head.is_empty() {
            raw
        } else {
            format!("{}\n\n{raw}", head.join("\n"))
        };
        let target = found.iter().flatten().find(|r| r.line == summary);
        let image = match target {
            Some(r) => shot(r).await.map(|png| BASE64.encode(&png)),
            None => None,
        };
        tracing::info!(session = %sid.0, tool = %tool, summary = %summary, "waiting for approval in the app");
        let (request, msg, answer) = self.approvals.open(sid, tool, summary, detail, image);
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

    /// Stands in for the registry: only run_command and unknown tools are risky here.
    fn risky(tool: &str) -> bool {
        matches!(tool, "run_command" | "mystery")
    }

    fn rule(tool: &str, pattern: Option<&str>, action: Action) -> Rule {
        Rule {
            tool: tool.into(),
            pattern: pattern.map(Into::into),
            action,
        }
    }

    #[test]
    fn unmatched_calls_follow_the_tool_risk() {
        let a = Approval::default();
        let run = json!({"command":"ls"});
        let ask = Verdict::Ask {
            tool: "run_command".into(),
            summary: "ls".into(),
        };
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "run_command", &run, &[], risky),
            ask
        );
        assert_eq!(
            verdict(&a, ApprovalMode::Auto, "run_command", &run, &[], risky),
            Verdict::Allow
        );
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "click", &json!({}), &[], risky),
            Verdict::Allow
        );
        assert!(matches!(
            verdict(&a, ApprovalMode::Always, "click", &json!({}), &[], risky),
            Verdict::Ask { tool, .. } if tool == "click"
        ));
        assert!(matches!(
            verdict(&a, ApprovalMode::Rules, "mystery", &json!({}), &[], risky),
            Verdict::Ask { .. }
        ));
    }

    #[test]
    fn a_fanout_asks_when_the_tool_it_runs_would() {
        let a = Approval::default();
        let fan = |tool: &str| json!({"tool":tool,"args":{"command":"ls"},"devices":["vm","pc"]});
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "fanout", &fan("run_command"), &[], risky),
            Verdict::Ask {
                tool: "run_command".into(),
                summary: "ls".into()
            }
        );
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "fanout", &fan("list_windows"), &[], risky),
            Verdict::Allow
        );
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
            assert_eq!(verdict(&a, mode, "run_command", &rm, &[], risky), Verdict::Deny);
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
            verdict(&a, ApprovalMode::Rules, "batch", &ask, &[], risky),
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
            verdict(&a, ApprovalMode::Rules, "batch", &deny, &[], risky),
            Verdict::Deny
        );
        let fine = batch(json!([{"tool":"wait","ms":1}]));
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "batch", &fine, &[], risky),
            Verdict::Allow
        );
    }

    const SEND: &str = r#"click [14] button "Delete account" in OUTLOOK.EXE: Inbox"#;

    #[test]
    fn rules_match_the_resolved_line_and_ask_with_it() {
        let click = json!({"window":"w3","element":14});
        let line = Some(SEND.to_string());
        for pattern in [r#"*"Delete account"*"#, "*button*", "*outlook.exe*"] {
            let a = Approval {
                rules: vec![rule("click", Some(pattern), Action::Ask)],
            };
            let v = verdict(&a, ApprovalMode::Rules, "click", &click, &[line.clone()], risky);
            assert_eq!(v, Verdict::Ask { tool: "click".into(), summary: SEND.into() }, "{pattern}");
            assert_eq!(
                verdict(&a, ApprovalMode::Rules, "click", &click, &[], risky),
                Verdict::Allow,
                "unresolved calls keep today's summary: {pattern}"
            );
        }
        let deny = Approval {
            rules: vec![rule("click", Some("*Delete*"), Action::Deny)],
        };
        assert_eq!(
            verdict(&deny, ApprovalMode::Auto, "click", &click, &[line], risky),
            Verdict::Deny
        );
    }

    #[test]
    fn typed_text_stays_matchable_when_resolved() {
        let a = Approval {
            rules: vec![rule("type_text", Some("secret*"), Action::Deny)],
        };
        let args = json!({"window":"w3","element":9,"text":"secret1"});
        let line = Some("type_text [9] edit in a.exe: t | typed: secret1".to_string());
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "type_text", &args, &[line], risky),
            Verdict::Deny
        );
    }

    #[test]
    fn batch_actions_take_their_own_resolved_line() {
        let a = Approval {
            rules: vec![rule("click", Some("*OUTLOOK.EXE*"), Action::Ask)],
        };
        let args = json!({"window":"w3","actions":[{"tool":"wait","ms":1},{"tool":"click","element":14}]});
        let resolved = [None, None, Some(SEND.to_string())];
        assert_eq!(
            verdict(&a, ApprovalMode::Rules, "batch", &args, &resolved, risky),
            Verdict::Ask { tool: "click".into(), summary: SEND.into() }
        );
    }

    #[test]
    fn resolving_is_skipped_when_nothing_could_ask() {
        let click = json!({"window":"w3","element":14});
        let a = Approval::default();
        assert!(!worth_resolving(&a, ApprovalMode::Rules, "click", &click, &risky));
        assert!(worth_resolving(&a, ApprovalMode::Always, "click", &click, &risky));
        assert!(worth_resolving(&a, ApprovalMode::Rules, "run_command", &click, &risky));
        let ruled = Approval {
            rules: vec![rule("click", None, Action::Deny)],
        };
        assert!(worth_resolving(&ruled, ApprovalMode::Auto, "click", &click, &risky));
    }
}
