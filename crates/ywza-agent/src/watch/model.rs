use serde::{Deserialize, Serialize};

pub const DEFAULT_MIN_GAP_S: u32 = 60;
pub const DEFAULT_MAX_PER_HOUR: u32 = 12;

fn min_gap() -> u32 {
    DEFAULT_MIN_GAP_S
}

fn max_per_hour() -> u32 {
    DEFAULT_MAX_PER_HOUR
}

/// A persistent trigger: `then` runs when `when` turns true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Watcher {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub when: Condition,
    pub then: Action,
    #[serde(default = "min_gap")]
    pub min_gap_s: u32,
    #[serde(default = "max_per_hour")]
    pub max_per_hour: u32,
    #[serde(default)]
    pub fired: u64,
    /// Unix time in milliseconds.
    #[serde(default)]
    pub last_fired_ms: Option<u64>,
}

/// Window conditions match a title substring (any case); window ids do not survive restarts.
/// Terminal ids are ephemeral too: owned terminals die with the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    WindowAppears {
        title: String,
    },
    WindowCloses {
        title: String,
    },
    ElementAppears {
        title: String,
        text: String,
    },
    /// The element whose name contains `text` changes name, value or state.
    ElementChanges {
        title: String,
        text: String,
    },
    Text {
        title: String,
        text: String,
        #[serde(default)]
        ocr: bool,
    },
    TermOutput {
        term: u32,
        text: String,
    },
    /// A command (or the terminal's process) finished; `code` None: any non-zero exit.
    TermExit {
        term: u32,
        code: Option<i32>,
    },
    /// `at` is "HH:MM" local time, daily.
    Schedule {
        every_min: Option<u32>,
        at: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// `text` None: a description of what happened.
    Notify { text: Option<String> },
    /// Starts a new AI session, on `device` (None: this machine).
    Prompt {
        text: String,
        device: Option<String>,
    },
    /// Replays the saved plan `name` with its default params, no AI involved.
    Plan { name: String },
}

impl Condition {
    pub fn needs_windows(&self) -> bool {
        use Condition::*;
        matches!(
            self,
            WindowAppears { .. }
                | WindowCloses { .. }
                | ElementAppears { .. }
                | ElementChanges { .. }
                | Text { .. }
        )
    }

    /// True for the moment it happens, not a state that holds: fires every time, no edge.
    pub fn is_event(&self) -> bool {
        use Condition::*;
        matches!(self, ElementChanges { .. } | TermExit { .. } | Schedule { .. })
    }

    pub fn describe(&self) -> String {
        use Condition::*;
        match self {
            WindowAppears { title } => format!("window {title:?} appears"),
            WindowCloses { title } => format!("window {title:?} closes"),
            ElementAppears { title, text } => format!("{text:?} appears in window {title:?}"),
            ElementChanges { title, text } => format!("{text:?} changes in window {title:?}"),
            Text { title, text, ocr } => {
                let ocr = if *ocr { " (ocr)" } else { "" };
                format!("text {text:?} in window {title:?}{ocr}")
            }
            TermOutput { term, text } => format!("t{term} output shows {text:?}"),
            TermExit { term, code: Some(c) } => format!("t{term} command exits with {c}"),
            TermExit { term, code: None } => format!("t{term} command exits non-zero"),
            Schedule {
                every_min: Some(m), ..
            } => format!("every {m} min"),
            Schedule { at: Some(a), .. } => format!("daily at {a}"),
            Schedule { .. } => "schedule".into(),
        }
    }
}

fn short(s: &str) -> String {
    match s.char_indices().nth(40) {
        Some((i, _)) => format!("{}...", &s[..i]),
        None => s.into(),
    }
}

impl Action {
    pub fn describe(&self) -> String {
        match self {
            Action::Notify { text: None } => "notify".into(),
            Action::Notify { text: Some(t) } => format!("notify {:?}", short(t)),
            Action::Prompt { text, device } => {
                let on = device.as_deref().map(|d| format!(" on {d}")).unwrap_or_default();
                format!("prompt {:?}{on}", short(text))
            }
            Action::Plan { name } => format!("run plan {name}"),
        }
    }
}

impl Watcher {
    /// `k3x9a on "build done" when window "Build" closes then notify (fired 2)`
    pub fn line(&self) -> String {
        format!(
            "{} {} {:?} when {} then {} (fired {})",
            self.id,
            if self.enabled { "on" } else { "off" },
            self.name,
            self.when.describe(),
            self.then.describe(),
            self.fired
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stored_shape_is_tagged_and_defaults_apply() {
        let w: Watcher = serde_json::from_value(json!({
            "id": "abc12", "name": "n", "enabled": true,
            "when": {"type": "term_exit", "term": 2, "code": null},
            "then": {"type": "notify", "text": null}
        }))
        .unwrap();
        assert_eq!(w.min_gap_s, 60);
        assert_eq!(w.max_per_hour, 12);
        assert_eq!(w.fired, 0);
        let v = serde_json::to_value(&w).unwrap();
        assert_eq!(v["when"]["type"], "term_exit");
        assert_eq!(v["then"]["type"], "notify");
        assert_eq!(serde_json::from_value::<Watcher>(v).unwrap(), w);
    }

    #[test]
    fn describes_in_one_line() {
        let w = Watcher {
            id: "k3x9a".into(),
            name: "build done".into(),
            enabled: false,
            when: Condition::WindowCloses {
                title: "Build".into(),
            },
            then: Action::Prompt {
                text: "summarize".into(),
                device: Some("vm".into()),
            },
            min_gap_s: 60,
            max_per_hour: 12,
            fired: 2,
            last_fired_ms: None,
        };
        assert_eq!(
            w.line(),
            "k3x9a off \"build done\" when window \"Build\" closes then prompt \"summarize\" on vm (fired 2)"
        );
        assert!(Condition::Schedule { every_min: None, at: Some("09:30".into()) }
            .describe()
            .contains("09:30"));
    }

    #[test]
    fn short_cuts_on_a_char_boundary() {
        assert_eq!(short(&"é".repeat(50)).chars().count(), 43);
        assert_eq!(short("abc"), "abc");
    }
}
