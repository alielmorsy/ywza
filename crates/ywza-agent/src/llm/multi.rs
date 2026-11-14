mod meta;

use std::sync::Arc;

use serde_json::{json, Value};
use ywza_proto::{SessionId, ToolInfo};

use super::{
    prompt::{devices_section, system_prompt},
    Executor,
};
use crate::{
    remote::Remote,
    tools::{Space, ToolOutput},
    Agent,
};

/// This machine plus paired devices: every tool gets a `device` argument that picks where it runs.
pub struct Multi<'a> {
    pub agent: &'a Agent,
    pub devices: Vec<Arc<Remote>>,
}

fn add_device_arg(schema: &mut Value, names: &[&str]) {
    schema["properties"]["device"] = json!({
        "type": "string",
        "enum": names,
        "description": "Device to run this call on; omit for this computer."
    });
}

fn take_device(args: &mut Value) -> Option<String> {
    let device = args.as_object_mut()?.remove("device")?;
    device.as_str().filter(|d| !d.is_empty()).map(String::from)
}

impl Executor for Multi<'_> {
    fn tools(&self) -> Vec<ToolInfo> {
        let names: Vec<&str> = self.devices.iter().map(|d| d.name()).collect();
        let mut tools = self.agent.tools.infos();
        for t in &mut tools {
            add_device_arg(&mut t.parameters, &names);
        }
        tools.extend(meta::infos(&names));
        tools
    }

    async fn system_prompt(&self) -> String {
        let local = system_prompt(&crate::tools::monitors_text().await);
        let devices: Vec<_> = self
            .devices
            .iter()
            .map(|d| {
                let (os, monitors) = d.describe();
                (d.name().to_string(), os, monitors)
            })
            .collect();
        local + &devices_section(&devices)
    }

    fn route(&self, args: &mut Value) -> Option<String> {
        take_device(args)
    }

    async fn call(
        &self,
        sid: SessionId,
        device: Option<&str>,
        name: &str,
        args: &Value,
    ) -> ToolOutput {
        match name {
            meta::COPY => self.copy(args).await,
            meta::FANOUT => self.fanout(sid, args).await,
            _ => self.run_on(sid, device, name, args).await,
        }
    }
}

impl Multi<'_> {
    /// One ordinary tool call, on this computer (`None`) or a paired device.
    async fn run_on(
        &self,
        sid: SessionId,
        device: Option<&str>,
        name: &str,
        args: &Value,
    ) -> ToolOutput {
        let Some(device) = device else {
            let space = Space {
                session: sid,
                sub: None,
            };
            return self.agent.run_tool(space, name, args).await;
        };
        let Some(remote) = self.devices.iter().find(|d| d.name() == device) else {
            let known: Vec<_> = self.devices.iter().map(|d| d.name()).collect();
            return ToolOutput::err(format!("unknown device `{device}`; use one of {known:?}"));
        };
        let mut out = remote.call(sid, Some(device), name, args).await;
        // Window ids repeat across devices.
        out.supersedes_key = out.supersedes_key.map(|k| format!("{device}:{k}"));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_arg_is_added_to_the_schema_and_stripped_from_calls() {
        let mut schema = json!({"type":"object","properties":{"x":{"type":"integer"}}});
        add_device_arg(&mut schema, &["vm", "pc"]);
        assert_eq!(schema["properties"]["device"]["enum"], json!(["vm", "pc"]));
        assert_eq!(schema["properties"]["x"]["type"], "integer");
        let mut none = json!({"type":"object"});
        add_device_arg(&mut none, &["vm"]);
        assert!(none["properties"]["device"].is_object());

        let mut args = json!({"window":"w1","device":"vm"});
        assert_eq!(take_device(&mut args).as_deref(), Some("vm"));
        assert_eq!(args, json!({"window":"w1"}));
        let mut local = json!({"window":"w1"});
        assert_eq!(take_device(&mut local), None);
        let mut empty = json!({"device":""});
        assert_eq!(take_device(&mut empty), None);
        assert_eq!(empty, json!({}));
    }
}
