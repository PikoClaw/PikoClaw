use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

pub struct SleepTool;

#[derive(Debug, Deserialize)]
struct SleepInput {
    #[serde(alias = "duration_ms")]
    ms: u64,
}

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &'static str {
        "Sleep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Sleep".to_string(),
            description: "Wait for a specified duration in milliseconds. \
                Use instead of Bash(sleep ...) — it doesn't hold a shell process \
                and can run concurrently with other tools. Max 300000ms (5 minutes)."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "ms": {
                        "type": "number",
                        "description": "Duration to sleep in milliseconds (max 300000 = 5 minutes)"
                    }
                }),
                required: vec!["ms".to_string()],
            },
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let params: SleepInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let duration_ms = params.ms.min(300_000);
        debug!(ms = duration_ms, "Sleeping");

        tokio::time::sleep(Duration::from_millis(duration_ms)).await;

        ToolResult::success(id, format!("Slept for {}ms.", duration_ms))
    }
}
