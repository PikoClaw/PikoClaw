use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolResult};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub cwd: PathBuf,
    pub cancellation: CancellationToken,
    pub env: std::collections::HashMap<String, String>,
}

impl ToolContext {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            cancellation: CancellationToken::new(),
            env: std::collections::HashMap::new(),
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;

    fn is_read_only(&self) -> bool {
        false
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        format!("run {} with input: {}", self.name(), input)
    }
}
