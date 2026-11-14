//! `ywza watch`: manage watchers on this machine's agent, or on a paired device with `--device`.

use anyhow::{anyhow, bail, Result};
use clap::{Args, Subcommand};
use serde_json::{json, Map, Value};
use ywza_proto::{client::call_tool, Control};

#[derive(Subcommand)]
pub enum WatchCmd {
    /// List watchers (`--json`: the full records).
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create a watcher from a JSON object of `watcher_create` arguments, or from flags:
    /// `ywza watch add --on window_appears --title Build --message "build window opened"`.
    Add(AddArgs),
    /// Turn a watcher on.
    On { id: String },
    /// Turn a watcher off.
    Off { id: String },
    /// Delete a watcher.
    Rm { id: String },
}

#[derive(Args, Default)]
pub struct AddArgs {
    /// Arguments as JSON, e.g. '{"on":"term_exit","term":"t2","then":"prompt","prompt":"why did it fail?"}'.
    json: Option<String>,
    /// window_appears, window_closes, element_appears, element_changes, text, term_output, term_exit or schedule.
    #[arg(long)]
    on: Option<String>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    ocr: bool,
    #[arg(long)]
    term: Option<String>,
    #[arg(long, allow_negative_numbers = true)]
    code: Option<i32>,
    #[arg(long)]
    every_min: Option<u32>,
    /// Daily at this local time, "HH:MM".
    #[arg(long)]
    at: Option<String>,
    /// notify (default) or prompt.
    #[arg(long)]
    then: Option<String>,
    #[arg(long)]
    message: Option<String>,
    #[arg(long)]
    prompt: Option<String>,
    /// Paired device a prompt session runs on (default: the watching machine).
    #[arg(long)]
    run_on: Option<String>,
    #[arg(long)]
    min_gap_s: Option<u32>,
    #[arg(long)]
    max_per_hour: Option<u32>,
    #[arg(long)]
    name: Option<String>,
}

impl AddArgs {
    fn into_args(self) -> Result<Value> {
        if let Some(j) = &self.json {
            let v: Value = serde_json::from_str(j).map_err(|e| anyhow!("bad JSON: {e}"))?;
            if !v.is_object() {
                bail!("the JSON must be an object");
            }
            return Ok(v);
        }
        if self.on.is_none() {
            bail!("give a JSON object or at least --on");
        }
        let mut m = Map::new();
        let mut put = |k: &str, v: Option<Value>| {
            if let Some(v) = v {
                m.insert(k.into(), v);
            }
        };
        put("on", self.on.map(Value::from));
        put("title", self.title.map(Value::from));
        put("text", self.text.map(Value::from));
        put("ocr", self.ocr.then_some(Value::Bool(true)));
        put("term", self.term.map(Value::from));
        put("code", self.code.map(Value::from));
        put("every_min", self.every_min.map(Value::from));
        put("at", self.at.map(Value::from));
        put("then", self.then.map(Value::from));
        put("message", self.message.map(Value::from));
        put("prompt", self.prompt.map(Value::from));
        put("run_on", self.run_on.map(Value::from));
        put("min_gap_s", self.min_gap_s.map(Value::from));
        put("max_per_hour", self.max_per_hour.map(Value::from));
        put("name", self.name.map(Value::from));
        Ok(Value::Object(m))
    }
}

pub async fn run(c: &mut Control, cmd: WatchCmd) -> Result<()> {
    let (tool, args) = match cmd {
        WatchCmd::List { json } => ("watcher_list", json!({ "json": json })),
        WatchCmd::Add(a) => ("watcher_create", a.into_args()?),
        WatchCmd::On { id } => ("watcher_set", json!({ "id": id, "enabled": true })),
        WatchCmd::Off { id } => ("watcher_set", json!({ "id": id, "enabled": false })),
        WatchCmd::Rm { id } => ("watcher_delete", json!({ "id": id })),
    };
    let r = call_tool(c, tool, args).await?;
    if !r.ok {
        bail!("{}", r.text);
    }
    println!("{}", r.text);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_become_tool_arguments() {
        let a = AddArgs {
            on: Some("term_exit".into()),
            term: Some("t2".into()),
            code: Some(-1),
            ocr: false,
            run_on: Some("vm".into()),
            ..Default::default()
        };
        assert_eq!(
            a.into_args().unwrap(),
            json!({"on": "term_exit", "term": "t2", "code": -1, "run_on": "vm"})
        );
    }

    #[test]
    fn json_passes_through_and_nothing_is_an_error() {
        let a = AddArgs {
            json: Some(r#"{"on":"schedule","every_min":5}"#.into()),
            ..Default::default()
        };
        assert_eq!(a.into_args().unwrap(), json!({"on": "schedule", "every_min": 5}));
        let bad = AddArgs {
            json: Some("[1]".into()),
            ..Default::default()
        };
        assert!(bad.into_args().is_err());
        assert!(AddArgs::default().into_args().is_err());
    }
}
