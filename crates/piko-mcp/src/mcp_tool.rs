use crate::client::McpClient;
use crate::protocol::McpContent;
use crate::protocol::McpToolDefinition;
use async_trait::async_trait;
use piko_tools::tool_trait::{Tool, ToolContext};
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use std::sync::Arc;

pub struct McpTool {
    client: Arc<McpClient>,
    tool_def: McpToolDefinition,
    qualified_name: String,
}

impl McpTool {
    pub fn new(client: Arc<McpClient>, tool_def: McpToolDefinition) -> Self {
        let qualified_name = format!("mcp__{}__{}", client.server_name(), tool_def.name);
        Self {
            client,
            tool_def,
            qualified_name,
        }
    }

    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &'static str {
        Box::leak(self.qualified_name.clone().into_boxed_str())
    }

    fn definition(&self) -> ToolDefinition {
        let schema = &self.tool_def.input_schema;
        let input_schema = ToolInputSchema {
            schema_type: schema
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("object")
                .to_string(),
            properties: schema
                .get("properties")
                .cloned()
                .unwrap_or(serde_json::json!({})),
            required: schema
                .get("required")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        ToolDefinition {
            name: self.qualified_name.clone(),
            description: self.tool_def.description.clone().unwrap_or_default(),
            input_schema,
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        match self.client.call_tool(&self.tool_def.name, input).await {
            Ok(result) => {
                let content = result
                    .content
                    .iter()
                    .map(|c| match c {
                        McpContent::Text { text } => text.clone(),
                        McpContent::Image { mime_type, .. } => {
                            format!("[image: {}]", mime_type)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if result.is_error {
                    ToolResult::error(String::new(), content)
                } else {
                    ToolResult::success(String::new(), content)
                }
            }
            Err(e) => ToolResult::error(String::new(), e.to_string()),
        }
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        format!(
            "MCP tool {} on server {} with input: {}",
            self.tool_def.name,
            self.client.server_name(),
            input
        )
    }
}

pub async fn load_mcp_tools(
    client: Arc<McpClient>,
) -> anyhow::Result<Vec<Arc<dyn Tool + Send + Sync>>> {
    let list = client.list_tools().await?;
    let tools: Vec<Arc<dyn Tool + Send + Sync>> = list
        .tools
        .into_iter()
        .map(|def| -> Arc<dyn Tool + Send + Sync> {
            Arc::new(McpTool::new(Arc::clone(&client), def))
        })
        .collect();
    Ok(tools)
}
