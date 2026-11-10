use std::future::Future;

use serde_json::Value;
use ywza_proto::ToolInfo;

use super::system_prompt;
use crate::{tools::ToolOutput, Agent};

/// Where the model's tool calls run: this machine, or a paired device (`crate::remote`).
pub trait Executor: Sync {
    fn tools(&self) -> Vec<ToolInfo>;
    fn system_prompt(&self) -> impl Future<Output = String> + Send;
    fn call(&self, name: &str, args: &Value) -> impl Future<Output = ToolOutput> + Send;
}

pub struct Local<'a>(pub &'a Agent);

impl Executor for Local<'_> {
    fn tools(&self) -> Vec<ToolInfo> {
        self.0.tools.infos()
    }

    async fn system_prompt(&self) -> String {
        system_prompt(&crate::tools::monitors_text().await)
    }

    async fn call(&self, name: &str, args: &Value) -> ToolOutput {
        self.0.run_tool(name, args).await
    }
}
