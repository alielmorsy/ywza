//! AI-facing tools. A tool is an args struct (`Deserialize + JsonSchema`, doc comment =
//! description) plus a `run`; the registry below is the only list.
//!
//! Desktop, terminal and OCR calls block (Win32/COM/helper processes), so they run in
//! `spawn_blocking`. COM needs no per-thread setup: ywza-desktop keeps the process MTA alive and
//! ywza-ocr calls `RoInitialize` itself.

mod act;
mod args;
mod batch;
mod ctx;
mod elements;
mod input;
mod ocr;
mod output;
mod schema;
mod screen;
mod terminal;
mod util;
mod wait;
mod windows;

use std::{future::Future, pin::Pin};

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

pub use args::{parse_window, ElementArg, TermArg, WindowArg};
pub use batch::{format_steps, parse_batch, Step, StepResult, BATCH_TOOLS};
pub use ctx::{Ctx, ToolSpec, Tools};
pub use output::{downscale, ToolOutput, MODEL_IMAGE_MAX};
pub use screen::{capture, monitors_text};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Tool: DeserializeOwned + JsonSchema + Send + 'static {
    const NAME: &'static str;
    fn run(self, cx: &Ctx) -> impl Future<Output = ToolOutput> + Send;
}

pub struct Entry {
    pub name: &'static str,
    pub description: String,
    pub schema: Value,
    pub call: for<'a> fn(Value, &'a Ctx) -> BoxFuture<'a, ToolOutput>,
}

pub fn entry<T: Tool>() -> Entry {
    let (description, schema) = schema::describe::<T>();
    Entry {
        name: T::NAME,
        description,
        schema,
        call: call::<T>,
    }
}

fn call<'a, T: Tool>(args: Value, cx: &'a Ctx) -> BoxFuture<'a, ToolOutput> {
    Box::pin(async move {
        // A tool without arguments may be called with `null`.
        let args = if args.is_null() { json!({}) } else { args };
        match T::deserialize(&args) {
            Ok(tool) => tool.run(cx).await,
            Err(e) => {
                let (_, schema) = schema::describe::<T>();
                ToolOutput::err(schema::bad_args(T::NAME, &e, &schema, &args))
            }
        }
    })
}

pub fn registry() -> Vec<Entry> {
    vec![
        entry::<windows::ListMonitors>(),
        entry::<windows::ListWindows>(),
        entry::<windows::FocusWindow>(),
        entry::<windows::MoveWindow>(),
        entry::<windows::CloseWindow>(),
        entry::<windows::Launch>(),
        entry::<elements::GetElements>(),
        entry::<input::Click>(),
        entry::<input::LongPress>(),
        entry::<input::TypeText>(),
        entry::<batch::Batch>(),
        entry::<input::PressKeys>(),
        entry::<input::Scroll>(),
        entry::<screen::Screenshot>(),
        entry::<terminal::TermList>(),
        entry::<terminal::TermStart>(),
        entry::<terminal::TermRead>(),
        entry::<terminal::TermWrite>(),
        entry::<terminal::TermKeys>(),
        entry::<util::RunCommand>(),
        entry::<wait::Wait>(),
        entry::<util::Notify>(),
    ]
}

#[cfg(test)]
mod tests {
    use ywza_proto::OcrEngine;

    use super::*;

    #[test]
    fn every_original_tool_is_registered_in_order() {
        let names: Vec<&str> = registry().iter().map(|e| e.name).collect();
        assert_eq!(
            names,
            [
                "list_monitors",
                "list_windows",
                "focus_window",
                "move_window",
                "close_window",
                "launch",
                "get_elements",
                "click",
                "long_press",
                "type_text",
                "batch",
                "press_keys",
                "scroll",
                "screenshot",
                "term_list",
                "term_start",
                "term_read",
                "term_write",
                "term_keys",
                "run_command",
                "wait",
                "notify",
            ]
        );
    }

    #[test]
    fn schemas_keep_the_original_fields() {
        let specs = Tools::default().specs();
        let spec = |n: &str| specs.iter().find(|s| s.name == n).unwrap();
        for s in &specs {
            assert_eq!(s.parameters["type"], "object", "{}", s.name);
            assert!(!s.description.is_empty(), "{}", s.name);
        }
        let click = &spec("click").parameters;
        assert_eq!(click["required"], json!(["window", "element"]));
        assert_eq!(
            click["properties"]["button"]["enum"],
            json!(["left", "right", "middle"])
        );
        assert_eq!(click["properties"]["count"]["default"], 1);
        let batch = &spec("batch").parameters;
        assert_eq!(batch["required"], json!(["actions"]));
        assert_eq!(batch["properties"]["actions"]["minItems"], 1);
        assert_eq!(
            batch["properties"]["actions"]["items"]["properties"]["tool"]["enum"],
            json!(BATCH_TOOLS)
        );
        assert_eq!(spec("wait").parameters["required"], json!(["ms"]));
        assert_eq!(spec("list_monitors").parameters["properties"], json!({}));
    }

    #[tokio::test]
    async fn bad_arguments_are_error_outputs_naming_the_field() {
        let t = Tools::default();
        let call = |name: &'static str, args: Value| {
            let t = &t;
            async move { t.call(name, &args, OcrEngine::Auto).await }
        };
        let out = call("click", json!({"window":"w1"})).await;
        assert!(!out.ok && out.text.contains("missing field `element`"), "{out:?}");
        let out = call("click", json!({"window":"w1","element":"x"})).await;
        assert!(!out.ok && out.text.contains("`element`"), "{out:?}");
        let out = call("wait", json!({"ms":"soon"})).await;
        assert!(!out.ok && out.text.contains("`ms`"), "{out:?}");
        let out = call("focus_window", json!({"window": true})).await;
        assert!(!out.ok && out.text.contains("window"), "{out:?}");
        let out = call("batch", json!({"window":"w1"})).await;
        assert!(!out.ok && out.text.contains("`actions`"), "{out:?}");
        let out = call("nope", json!({})).await;
        assert_eq!(out.text, "unknown tool `nope`");
        let out = call("list_monitors", Value::Null).await;
        assert!(!out.text.starts_with("bad arguments"), "{out:?}");
    }
}
