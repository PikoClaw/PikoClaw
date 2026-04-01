use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "WebSearch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "WebSearch".to_string(),
            description: "Search the web for current information. Returns relevant search results. Use for finding recent news, documentation, or facts not in training data.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                }),
                required: vec!["query".to_string()],
            },
        }
    }

    fn is_web_search(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        ToolResult::error(
            id,
            "WebSearch is handled natively by the Anthropic API".to_string(),
        )
    }
}
