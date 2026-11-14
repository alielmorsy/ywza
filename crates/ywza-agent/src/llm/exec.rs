use std::future::Future;

use serde_json::Value;
use ywza_proto::{SessionId, ToolInfo};

use super::system_prompt;
use crate::{
    tools::{Space, ToolOutput},
    Agent,
};

/// Where the model's tool calls run: this machine, or a paired device (`crate::remote`).
pub trait Executor: Sync {
    fn tools(&self) -> Vec<ToolInfo>;
    fn system_prompt(&self) -> impl Future<Output = String> + Send;
    /// Takes the routing `device` argument out of `args`; None: no routing (the default).
    fn route(&self, _args: &mut Value) -> Option<String> {
        None
    }
    /// `device` comes from `route`.
    fn call(
        &self,
        sid: SessionId,
        device: Option<&str>,
        name: &str,
        args: &Value,
    ) -> impl Future<Output = ToolOutput> + Send;
}

pub struct Local<'a>(pub &'a Agent);

impl Executor for Local<'_> {
    fn tools(&self) -> Vec<ToolInfo> {
        self.0.tools.infos()
    }

    async fn system_prompt(&self) -> String {
        system_prompt(&crate::tools::monitors_text().await)
    }

    async fn call(
        &self,
        sid: SessionId,
        _device: Option<&str>,
        name: &str,
        args: &Value,
    ) -> ToolOutput {
        let space = Space {
            session: sid,
            sub: None,
        };
        self.0.run_tool(space, name, args).await
    }
}
