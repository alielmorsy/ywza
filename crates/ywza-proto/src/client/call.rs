use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::{read_frame, write_frame, AgentMsg, ClientMsg, Control};

/// What a directly called tool answered.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolReply {
    pub ok: bool,
    pub text: String,
    pub png_base64: Option<String>,
}

/// Runs one tool on the connected agent (`CallTool`, no approval step). Events and images the
/// agent pushes meanwhile are skipped.
pub async fn call_tool(c: &mut Control, name: &str, args: Value) -> Result<ToolReply> {
    let call = 1;
    let msg = ClientMsg::CallTool {
        call,
        name: name.into(),
        args,
        space: None,
    };
    write_frame(&mut c.send, &msg).await?;
    loop {
        match read_frame::<_, AgentMsg>(&mut c.recv).await? {
            AgentMsg::Error { message } => bail!("agent: {message}"),
            AgentMsg::ToolResult {
                call: id,
                ok,
                text,
                png_base64,
            } if id == call => {
                return Ok(ToolReply {
                    ok,
                    text,
                    png_base64,
                })
            }
            _ => {}
        }
    }
}

/// One device's part of a fan-out result: `[vm] ok` and its output, or `[pc] error: why`.
/// Images are dropped with a note.
pub fn fanout_section(device: &str, ok: bool, text: &str, image: bool) -> String {
    let note = if image { "\n(image omitted)" } else { "" };
    if ok {
        format!("[{device}] ok\n{text}{note}")
    } else {
        format!("[{device}] error: {text}{note}")
    }
}

/// The devices to fan out to: `all` is every paired device, else a comma list. `local` (this
/// machine) is allowed in a list.
pub fn parse_targets(spec: &str, paired: &[String]) -> Result<Vec<String>> {
    if spec == "all" {
        if paired.is_empty() {
            bail!("no paired devices");
        }
        return Ok(paired.to_vec());
    }
    let mut out: Vec<String> = Vec::new();
    for n in spec.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if n != "local" && !super::valid_device_name(n) {
            return Err(anyhow!("bad device name `{n}`"));
        }
        if !out.iter().any(|o| o == n) {
            out.push(n.into());
        }
    }
    if out.is_empty() {
        bail!("no devices given");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_say_ok_or_error_and_note_dropped_images() {
        assert_eq!(fanout_section("vm", true, "hi", false), "[vm] ok\nhi");
        assert_eq!(
            fanout_section("pc", false, "gone", true),
            "[pc] error: gone\n(image omitted)"
        );
    }

    #[test]
    fn targets_are_a_list_or_everything_paired() {
        let paired = vec!["vm".to_string(), "pc".to_string()];
        assert_eq!(parse_targets("all", &paired).unwrap(), paired);
        assert_eq!(
            parse_targets("vm, local,vm", &paired).unwrap(),
            ["vm", "local"]
        );
        assert!(parse_targets("a/b", &paired).is_err());
        assert!(parse_targets(" , ", &paired).is_err());
        assert!(parse_targets("all", &[]).is_err());
    }
}
