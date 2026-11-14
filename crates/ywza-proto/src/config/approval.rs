use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Allow,
    Ask,
    Deny,
}

/// How a session applies the rules. Per session, set by local clients on `Prompt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// The rules decide.
    #[default]
    Rules,
    /// `ask` becomes `allow`; `deny` stays.
    Auto,
    /// `allow` becomes `ask`; `deny` stays.
    Always,
}

impl ApprovalMode {
    pub fn apply(self, action: Action) -> Action {
        match (self, action) {
            (Self::Auto, Action::Ask) => Action::Allow,
            (Self::Always, Action::Allow) => Action::Ask,
            _ => action,
        }
    }
}

/// `pattern` is a case-insensitive glob (`*`, `?`) over the call's one-line summary; absent
/// matches every call of `tool`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub action: Action,
}

/// Overrides on top of each tool's own default (risky tools ask, the rest run).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Approval {
    pub rules: Vec<Rule>,
}

impl Approval {
    /// The first matching rule's action; None when no rule matches.
    pub fn decide(&self, tool: &str, summary: &str) -> Option<Action> {
        self.decide_any(tool, &[summary])
    }

    /// Like `decide`, a rule's pattern matching if it matches any of the `summaries`.
    pub fn decide_any(&self, tool: &str, summaries: &[&str]) -> Option<Action> {
        self.rules
            .iter()
            .find(|r| {
                r.tool == tool
                    && r.pattern
                        .as_deref()
                        .is_none_or(|p| summaries.iter().any(|s| glob(p, s)))
            })
            .map(|r| r.action)
    }

    /// "Always allow this tool": a pattern-less allow rule that goes before every other rule.
    pub fn remember(&mut self, tool: &str) {
        let rule = Rule {
            tool: tool.into(),
            pattern: None,
            action: Action::Allow,
        };
        if self.rules.first() != Some(&rule) {
            self.rules.insert(0, rule);
        }
    }
}

/// Case-insensitive glob: `*` is any run of characters, `?` one character.
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(tool: &str, pattern: Option<&str>, action: Action) -> Rule {
        Rule {
            tool: tool.into(),
            pattern: pattern.map(Into::into),
            action,
        }
    }

    #[test]
    fn glob_wildcards_and_case() {
        assert!(glob("*", ""));
        assert!(glob("rm *", "RM -rf /tmp"));
        assert!(glob("git * --force*", "git push origin --force-with-lease"));
        assert!(glob("a?c", "abc") && !glob("a?c", "ac") && !glob("a?c", "abbc"));
        assert!(glob("*end", "the end") && !glob("*end", "the end."));
        assert!(!glob("", "x") && glob("", ""));
    }

    #[test]
    fn first_match_wins_and_unmatched_is_none() {
        let a = Approval {
            rules: vec![
                rule("run_command", Some("echo *"), Action::Allow),
                rule("run_command", Some("rm *"), Action::Deny),
                rule("run_command", None, Action::Ask),
            ],
        };
        assert_eq!(a.decide("run_command", "echo hi"), Some(Action::Allow));
        assert_eq!(a.decide("run_command", "RM -rf x"), Some(Action::Deny));
        assert_eq!(a.decide("run_command", "ls"), Some(Action::Ask));
        assert_eq!(a.decide("click", "rm x"), None);
        assert_eq!(a.decide_any("run_command", &["ls", "rm y"]), Some(Action::Deny));
        assert_eq!(a.decide_any("run_command", &[]), Some(Action::Ask));
    }

    #[test]
    fn modes_remap_ask_and_allow_but_never_deny() {
        use Action::*;
        use ApprovalMode::*;
        for (mode, want) in [
            (Rules, [Allow, Ask, Deny]),
            (Auto, [Allow, Allow, Deny]),
            (Always, [Ask, Ask, Deny]),
        ] {
            assert_eq!([Allow, Ask, Deny].map(|a| mode.apply(a)), want, "{mode:?}");
        }
    }

    #[test]
    fn remember_goes_first_and_is_not_repeated() {
        let mut a = Approval::default();
        a.remember("run_command");
        a.remember("run_command");
        assert_eq!(a.rules.len(), 1);
        assert_eq!(a.decide("run_command", "ls"), Some(Action::Allow));
        assert_eq!(a.rules[0], rule("run_command", None, Action::Allow));
    }

    #[test]
    fn json_shapes() {
        assert_eq!(
            serde_json::to_value(rule("run_command", None, Action::Ask)).unwrap(),
            json!({"tool":"run_command","action":"ask"})
        );
        assert_eq!(
            serde_json::to_value(rule("type_text", Some("a*"), Action::Deny)).unwrap(),
            json!({"tool":"type_text","pattern":"a*","action":"deny"})
        );
        assert_eq!(
            serde_json::to_value([
                ApprovalMode::Rules,
                ApprovalMode::Auto,
                ApprovalMode::Always
            ])
            .unwrap(),
            json!(["rules", "auto", "always"])
        );
        assert_eq!(ApprovalMode::default(), ApprovalMode::Rules);
    }
}
