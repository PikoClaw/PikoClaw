use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};

pub struct SyntheticOutputTool;

#[async_trait]
impl Tool for SyntheticOutputTool {
    fn name(&self) -> &'static str {
        "StructuredOutput"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "StructuredOutput".to_string(),
            description: "Return structured output in the requested format. \
                Use this tool to return your final response as structured JSON. \
                You MUST call this tool exactly once at the end of your response \
                to provide the structured output."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({}),
                required: vec![],
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

        if !input.is_object() {
            return ToolResult::error(id, "StructuredOutput requires a JSON object as input.");
        }

        ToolResult::success(id, "Structured output provided successfully")
    }
}
