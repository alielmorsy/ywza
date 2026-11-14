use anyhow::Result;
use serde_json::Value;
use ywza_proto::client::{self, ToolReply};

use crate::CLIENT_NAME;

/// Runs one tool on `device` (`local`: this machine) over its own connection.
pub async fn call(device: &str, tool: &str, args: Value) -> Result<ToolReply> {
    let (ep, mut c, _) = match device {
        "local" => client::open_local(CLIENT_NAME).await?,
        name => client::open_device(name, CLIENT_NAME).await?,
    };
    let r = client::call_tool(&mut c, tool, args).await;
    client::close(ep, &c).await;
    r
}
