//! Tools only a multi-device session has: they reach several devices themselves, so they stay
//! out of the registry (a peer calling this agent's tools cannot reach its other devices).

use std::task::Poll;

use serde::Deserialize;
use serde_json::{json, Value};
use ywza_proto::{
    client::{self, copy::Stats, fanout_section},
    SessionId, ToolInfo,
};

use super::Multi;
use crate::tools::{BoxFuture, ToolOutput};

pub const COPY: &str = "copy";
pub const FANOUT: &str = "fanout";

pub fn infos(names: &[&str]) -> Vec<ToolInfo> {
    let mut targets = names.to_vec();
    targets.push("local");
    vec![
        ToolInfo {
            name: COPY.into(),
            risky: true,
            description: "Copy a file or folder between this computer and a paired device, or \
between two devices. A path is `device:path`, `local:path` or a plain path on this computer. An \
existing folder as destination receives the source inside it; on a device, relative paths and ~ \
are under its user's home."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "src": {"type": "string", "description": "Source file or folder."},
                    "dst": {"type": "string", "description": "Destination path."},
                    "recursive": {"type": "boolean", "description": "Copy folders with their content (default true)."}
                },
                "required": ["src", "dst"]
            }),
        },
        ToolInfo {
            name: FANOUT.into(),
            risky: false,
            description: "Run one tool on several devices in parallel and get one section per \
device (`[vm] ok` and its output, or `[pc] error: why`). Images are dropped. `devices` may \
include `local` for this computer."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tool": {"type": "string", "description": "Tool name, e.g. find_app."},
                    "args": {"type": "object", "description": "Arguments of that tool."},
                    "devices": {"type": "array", "items": {"type": "string", "enum": targets}}
                },
                "required": ["tool", "devices"]
            }),
        },
    ]
}

#[derive(Deserialize)]
struct CopyArgs {
    src: String,
    dst: String,
    #[serde(default = "yes")]
    recursive: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct FanArgs {
    tool: String,
    #[serde(default)]
    args: Value,
    devices: Vec<String>,
}

fn bad(tool: &str, e: serde_json::Error) -> ToolOutput {
    ToolOutput::err(format!("bad arguments for {tool}: {e}"))
}

fn copy_report(st: &Stats) -> String {
    let mut s = format!("copied {} entries, {} bytes", st.files, st.bytes);
    for e in &st.errors {
        s.push_str(&format!("\nerror: {e}"));
    }
    s
}

/// `devices` without repeats, in order.
fn unique(devices: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in devices {
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

impl Multi<'_> {
    fn known(&self, name: &str) -> bool {
        self.devices.iter().any(|d| d.name() == name)
    }

    fn unknown(&self, name: &str) -> ToolOutput {
        let known: Vec<_> = self.devices.iter().map(|d| d.name()).collect();
        ToolOutput::err(format!("unknown device `{name}`; use one of {known:?}"))
    }

    pub(super) async fn copy(&self, args: &Value) -> ToolOutput {
        let a: CopyArgs = match serde_json::from_value(args.clone()) {
            Ok(a) => a,
            Err(e) => return bad(COPY, e),
        };
        for name in client::copy::device_names(&[a.src.clone(), a.dst.clone()]) {
            if !self.known(&name) {
                return self.unknown(&name);
            }
        }
        let Some(first) = self.devices.first() else {
            return ToolOutput::err("no devices");
        };
        match client::copy::copy(first.endpoint(), &[a.src], &a.dst, a.recursive).await {
            Ok(st) => ToolOutput {
                ok: st.errors.is_empty(),
                text: copy_report(&st),
                ..Default::default()
            },
            Err(e) => ToolOutput::err(format!("{e:#}")),
        }
    }

    pub(super) async fn fanout(&self, sid: SessionId, args: &Value) -> ToolOutput {
        let f: FanArgs = match serde_json::from_value(args.clone()) {
            Ok(f) => f,
            Err(e) => return bad(FANOUT, e),
        };
        let devices = unique(f.devices);
        if devices.is_empty() {
            return ToolOutput::err("fanout needs at least one device");
        }
        if let Some(d) = devices.iter().find(|d| *d != "local" && !self.known(d)) {
            return self.unknown(d);
        }
        let call_args = if f.args.is_null() { json!({}) } else { f.args };
        let runs: Vec<BoxFuture<'_, ToolOutput>> = devices
            .iter()
            .map(|d| {
                let device = (d != "local").then_some(d.as_str());
                Box::pin(self.run_on(sid, device, &f.tool, &call_args)) as BoxFuture<'_, ToolOutput>
            })
            .collect();
        let outs = join_all(runs).await;
        let sections: Vec<String> = devices
            .iter()
            .zip(&outs)
            .map(|(d, o)| fanout_section(d, o.ok, &o.text, o.image_png.is_some()))
            .collect();
        ToolOutput {
            ok: outs.iter().any(|o| o.ok),
            text: sections.join("\n\n"),
            ..Default::default()
        }
    }
}

/// Polls every future on this task until all are done; results keep the input order.
async fn join_all<T>(mut futs: Vec<BoxFuture<'_, T>>) -> Vec<T> {
    let mut outs: Vec<Option<T>> = futs.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (f, o) in futs.iter_mut().zip(&mut outs) {
            if o.is_none() {
                match f.as_mut().poll(cx) {
                    Poll::Ready(v) => *o = Some(v),
                    Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
    outs.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn both_tools_are_offered_without_a_device_arg() {
        let tools = infos(&["vm", "pc"]);
        assert_eq!(tools[0].name, "copy");
        assert!(tools[0].risky && !tools[1].risky);
        assert!(tools
            .iter()
            .all(|t| t.parameters["properties"]["device"].is_null()));
        let devices = &tools[1].parameters["properties"]["devices"]["items"]["enum"];
        assert_eq!(devices, &json!(["vm", "pc", "local"]));
    }

    #[test]
    fn copy_report_lists_entry_errors() {
        let st = Stats {
            files: 2,
            bytes: 10,
            errors: vec!["vm:/x: denied".into()],
        };
        assert_eq!(
            copy_report(&st),
            "copied 2 entries, 10 bytes\nerror: vm:/x: denied"
        );
        let a: CopyArgs = serde_json::from_value(json!({"src":"a","dst":"vm:b"})).unwrap();
        assert!(a.recursive);
        assert_eq!(unique(vec!["a".into(), "b".into(), "a".into()]), ["a", "b"]);
    }

    #[tokio::test]
    async fn join_all_runs_together_and_keeps_order() {
        let slow = Duration::from_millis(80);
        let futs: Vec<BoxFuture<'_, u32>> = vec![
            Box::pin(async move {
                tokio::time::sleep(slow).await;
                1
            }),
            Box::pin(async move {
                tokio::time::sleep(slow).await;
                2
            }),
        ];
        let start = std::time::Instant::now();
        assert_eq!(join_all(futs).await, [1, 2]);
        assert!(start.elapsed() < slow * 2);
    }
}
