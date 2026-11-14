//! Tools that manage watchers (persistent triggers, see `crate::watch`).

use schemars::JsonSchema;
use serde::Deserialize;

use super::{Ctx, TermArg, Tool, ToolOutput};
use crate::watch::{
    self, parse_at, Action, Condition, Watcher, DEFAULT_MAX_PER_HOUR, DEFAULT_MIN_GAP_S,
};

const NAME_MAX: usize = 80;
const GAP_MAX: u32 = 86_400;
const PER_HOUR_MAX: u32 = 3_600;

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum On {
    WindowAppears,
    WindowCloses,
    ElementAppears,
    ElementChanges,
    Text,
    TermOutput,
    TermExit,
    Schedule,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Then {
    #[default]
    Notify,
    Prompt,
    Plan,
}

impl On {
    fn name(self) -> &'static str {
        match self {
            On::WindowAppears => "window_appears",
            On::WindowCloses => "window_closes",
            On::ElementAppears => "element_appears",
            On::ElementChanges => "element_changes",
            On::Text => "text",
            On::TermOutput => "term_output",
            On::TermExit => "term_exit",
            On::Schedule => "schedule",
        }
    }
}

/// Create a watcher: a trigger that keeps running on this device, costs no tokens, and fires when its condition turns from false to true (not while it stays true). Checked about once a second. `on`: window_appears / window_closes (`title`: part of a window title, any case); element_appears (`title` + `text`: an element whose name or value contains it); element_changes (`title` + `text`: the element whose name contains `text` changes name, value or state); text (`title` + `text`, with `ocr` also text read from the pixels); term_output (`term` + `text`); term_exit (`term`: a command there finished with exit code `code`, default any non-zero); schedule (`every_min`, or `at` "HH:MM" local time daily). Terminal ids die with the agent, so term_* watchers only last while that terminal does. `then`: notify (default; shows `message`, or a description of what happened) or prompt (starts a new AI session with `prompt`, on the paired device `run_on` if given) or plan (replays the saved plan `plan` with its default params, no AI). At most one fire per `min_gap_s` (default 60); more than `max_per_hour` (default 12) fires in an hour turns it off. Up to 50 watchers.
#[derive(Deserialize, JsonSchema)]
pub struct WatcherCreate {
    on: On,
    /// Window title substring (any case) for window, element and text conditions.
    title: Option<String>,
    /// Text to look for (element_appears, element_changes, text, term_output).
    text: Option<String>,
    /// With `text`: also read the window's pixels (slower, tolerates OCR misreads).
    #[serde(default)]
    ocr: bool,
    /// Terminal id from term_list/term_start, e.g. "t2".
    term: Option<TermArg>,
    /// term_exit: fire only for this exit code (default: any non-zero).
    code: Option<i32>,
    /// schedule: every this many minutes (at least 1).
    every_min: Option<u32>,
    /// schedule: daily at this local time, "HH:MM" (24 h).
    at: Option<String>,
    #[serde(default)]
    then: Then,
    /// notify: the text to show (default: what happened).
    message: Option<String>,
    /// prompt: what the new AI session should do.
    prompt: Option<String>,
    /// prompt: a paired device to run the session on (default: this machine).
    run_on: Option<String>,
    /// plan: the saved plan to replay (see list_plans).
    plan: Option<String>,
    /// Minimum seconds between fires (default 60).
    min_gap_s: Option<u32>,
    /// Fires per hour before it turns itself off (default 12).
    max_per_hour: Option<u32>,
    /// A label (default: the condition).
    name: Option<String>,
}

fn need(v: &Option<String>, what: &str, ctx: &str) -> Result<String, String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| format!("`{ctx}` needs `{what}`"))
}

impl WatcherCreate {
    fn build(&self) -> Result<Watcher, String> {
        let on = self.on.name();
        let title = || need(&self.title, "title", on);
        let text = || need(&self.text, "text", on);
        let term = || {
            self.term
                .map(|t| t.0 .0)
                .ok_or_else(|| format!("`{on}` needs `term`"))
        };
        let when = match self.on {
            On::WindowAppears => Condition::WindowAppears { title: title()? },
            On::WindowCloses => Condition::WindowCloses { title: title()? },
            On::ElementAppears => Condition::ElementAppears {
                title: title()?,
                text: text()?,
            },
            On::ElementChanges => Condition::ElementChanges {
                title: title()?,
                text: text()?,
            },
            On::Text => Condition::Text {
                title: title()?,
                text: text()?,
                ocr: self.ocr,
            },
            On::TermOutput => Condition::TermOutput {
                term: term()?,
                text: text()?,
            },
            On::TermExit => Condition::TermExit {
                term: term()?,
                code: self.code,
            },
            On::Schedule => schedule(self.every_min, self.at.as_deref())?,
        };
        let then = match self.then {
            Then::Notify => Action::Notify {
                text: self
                    .message
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from),
            },
            Then::Prompt => {
                let device = self.run_on.as_deref().map(str::trim).filter(|d| !d.is_empty());
                if device.is_some_and(|d| !ywza_proto::client::valid_device_name(d)) {
                    return Err("`run_on` must be a paired device name (omit it for this machine)".into());
                }
                Action::Prompt {
                    text: need(&self.prompt, "prompt", "then: prompt")?,
                    device: device.map(String::from),
                }
            }
            Then::Plan => {
                let name = need(&self.plan, "plan", "then: plan")?;
                if !crate::plans::store::valid_name(&name) {
                    return Err(format!("no valid plan name `{name}`; see list_plans"));
                }
                Action::Plan { name }
            }
        };
        let min_gap_s = self.min_gap_s.unwrap_or(DEFAULT_MIN_GAP_S);
        let max_per_hour = self.max_per_hour.unwrap_or(DEFAULT_MAX_PER_HOUR);
        if min_gap_s > GAP_MAX {
            return Err(format!("`min_gap_s` is at most {GAP_MAX}"));
        }
        if !(1..=PER_HOUR_MAX).contains(&max_per_hour) {
            return Err(format!("`max_per_hour` must be 1 to {PER_HOUR_MAX}"));
        }
        let name: String = match self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => n.chars().take(NAME_MAX).collect(),
            None => when.describe().chars().take(NAME_MAX).collect(),
        };
        Ok(Watcher {
            id: String::new(),
            name,
            enabled: true,
            when,
            then,
            min_gap_s,
            max_per_hour,
            fired: 0,
            last_fired_ms: None,
        })
    }
}

fn schedule(every_min: Option<u32>, at: Option<&str>) -> Result<Condition, String> {
    match (every_min, at) {
        (Some(_), Some(_)) => Err("`schedule` takes `every_min` or `at`, not both".into()),
        (Some(0), None) => Err("`every_min` must be at least 1".into()),
        (Some(m), None) => Ok(Condition::Schedule {
            every_min: Some(m),
            at: None,
        }),
        (None, Some(a)) => {
            let t = parse_at(a).ok_or("`at` must be a local time like \"09:30\"")?;
            Ok(Condition::Schedule {
                every_min: None,
                at: Some(t.format("%H:%M").to_string()),
            })
        }
        (None, None) => Err("`schedule` needs `every_min` or `at`".into()),
    }
}

impl Tool for WatcherCreate {
    const NAME: &'static str = "watcher_create";
    const RISKY: bool = true;

    async fn run(self, _: &Ctx) -> ToolOutput {
        let made = self
            .build()
            .and_then(|w| watch::store()?.add(w));
        match made {
            Ok(w) => ToolOutput::ok(format!("created {}", w.line())),
            Err(e) => ToolOutput::err(e),
        }
    }
}

/// List watchers, one line each: id, on/off, name, condition, action, times fired. `json`: a JSON array of the full records, for apps.
#[derive(Deserialize, JsonSchema)]
pub struct WatcherList {
    #[serde(default)]
    json: bool,
}

impl Tool for WatcherList {
    const NAME: &'static str = "watcher_list";

    async fn run(self, _: &Ctx) -> ToolOutput {
        let list = match watch::store() {
            Ok(s) => s.list(),
            Err(e) => return ToolOutput::err(e),
        };
        if self.json {
            return match serde_json::to_string(&list) {
                Ok(t) => ToolOutput::ok(t),
                Err(e) => ToolOutput::err(e.to_string()),
            };
        }
        if list.is_empty() {
            return ToolOutput::ok("no watchers");
        }
        let lines: Vec<String> = list.iter().map(Watcher::line).collect();
        ToolOutput::ok(lines.join("\n"))
    }
}

/// Turn a watcher on or off. Turning it on starts it fresh: a condition that already holds does not fire.
#[derive(Deserialize, JsonSchema)]
pub struct WatcherSet {
    /// Watcher id from watcher_list.
    id: String,
    enabled: bool,
}

impl Tool for WatcherSet {
    const NAME: &'static str = "watcher_set";

    async fn run(self, _: &Ctx) -> ToolOutput {
        match watch::store().and_then(|s| s.set_enabled(self.id.trim(), self.enabled)) {
            Ok(w) => ToolOutput::ok(w.line()),
            Err(e) => ToolOutput::err(e),
        }
    }
}

/// Delete a watcher.
#[derive(Deserialize, JsonSchema)]
pub struct WatcherDelete {
    /// Watcher id from watcher_list.
    id: String,
}

impl Tool for WatcherDelete {
    const NAME: &'static str = "watcher_delete";

    async fn run(self, _: &Ctx) -> ToolOutput {
        match watch::store().and_then(|s| s.remove(self.id.trim())) {
            Ok(w) => ToolOutput::ok(format!("deleted {}", w.line())),
            Err(e) => ToolOutput::err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    fn build(args: Value) -> Result<Watcher, String> {
        serde_json::from_value::<WatcherCreate>(args)
            .map_err(|e| e.to_string())?
            .build()
    }

    #[test]
    fn builds_each_condition_with_defaults() {
        let w = build(json!({"on": "window_appears", "title": " Build "})).unwrap();
        assert_eq!(w.when, Condition::WindowAppears { title: "Build".into() });
        assert_eq!((w.min_gap_s, w.max_per_hour), (60, 12));
        assert_eq!(w.then, Action::Notify { text: None });
        assert!(w.name.contains("Build"));
        let w = build(json!({"on": "term_exit", "term": "t3", "then": "prompt", "prompt": "look", "run_on": "vm"})).unwrap();
        assert_eq!(w.when, Condition::TermExit { term: 3, code: None });
        assert_eq!(
            w.then,
            Action::Prompt {
                text: "look".into(),
                device: Some("vm".into())
            }
        );
        let w = build(json!({"on": "text", "title": "a", "text": "b", "ocr": true, "name": "n"})).unwrap();
        assert_eq!(w.name, "n");
        assert!(matches!(w.when, Condition::Text { ocr: true, .. }));
    }

    #[test]
    fn missing_pieces_name_the_argument() {
        let e = build(json!({"on": "element_changes", "title": "a"})).unwrap_err();
        assert!(e.contains("`text`") && e.contains("element_changes"), "{e}");
        assert!(build(json!({"on": "term_output", "text": "x"})).unwrap_err().contains("`term`"));
        assert!(build(json!({"on": "window_closes", "title": "  "})).unwrap_err().contains("`title`"));
        assert!(build(json!({"on": "window_closes", "title": "a", "then": "prompt"}))
            .unwrap_err()
            .contains("`prompt`"));
        assert!(build(json!({"on": "nope"})).is_err());
    }

    #[test]
    fn schedules_take_exactly_one_form() {
        let w = build(json!({"on": "schedule", "at": "9:05"})).unwrap();
        assert!(matches!(w.when, Condition::Schedule { at: Some(a), .. } if a == "09:05"));
        assert!(build(json!({"on": "schedule"})).is_err());
        assert!(build(json!({"on": "schedule", "every_min": 0})).is_err());
        assert!(build(json!({"on": "schedule", "every_min": 5, "at": "09:00"})).is_err());
        assert!(build(json!({"on": "schedule", "at": "25:00"})).is_err());
        assert!(build(json!({"on": "schedule", "every_min": 5})).is_ok());
        assert!(build(json!({"on": "schedule", "at": "09:30"})).is_ok());
    }

    #[test]
    fn limits_and_devices_are_checked() {
        let base = json!({"on": "window_appears", "title": "a"});
        let with = |k: &str, v: Value| {
            let mut a = base.clone();
            a[k] = v;
            build(a)
        };
        assert!(with("max_per_hour", json!(0)).is_err());
        assert!(with("max_per_hour", json!(100_000)).is_err());
        assert!(with("min_gap_s", json!(1_000_000)).is_err());
        assert_eq!(with("min_gap_s", json!(0)).unwrap().min_gap_s, 0);
        let mut a = base.clone();
        a["then"] = json!("prompt");
        a["prompt"] = json!("x");
        a["run_on"] = json!("bad/name");
        assert!(build(a).unwrap_err().contains("`run_on`"));
    }
}
