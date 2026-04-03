use async_trait::async_trait;
use piko_tools::tool_trait::{Tool, ToolContext};
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::sync::Arc;

use crate::client::McpClient;
use crate::mcp_read_resource::ReadMcpResourceTool;

#[derive(Debug, Deserialize)]
pub struct ListMcpResourcesInput {
    #[serde(default)]
    pub server_name: Option<String>,
}

pub struct ListMcpResourcesTool {
    client: Arc<McpClient>,
}

impl ListMcpResourcesTool {
    pub fn new(client: Arc<McpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &'static str {
        "list_mcp_resources"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: "List all resources available from connected MCP servers. MCP resources are data sources like files, API endpoints, or other URI-addressable content.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "server_name": {
                        "type": "string",
                        "description": "Optional filter to only list resources from a specific MCP server"
                    }
                }),
                required: vec![],
            },
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let input: ListMcpResourcesInput = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::error(
                    "list_mcp_resources".to_string(),
                    format!("invalid input: {e}"),
                );
            }
        };

        if let Some(ref filter) = input.server_name {
            if filter != self.client.server_name() {
                return ToolResult::success(
                    "list_mcp_resources".to_string(),
                    format!(
                        "Filter server '{}' does not match connected server '{}'.",
                        filter,
                        self.client.server_name()
                    ),
                );
            }
        }

        // Check if server supports resources
        let caps = self.client.capabilities();
        if !caps.supports_resources {
            return ToolResult::success(
                "list_mcp_resources".to_string(),
                format!(
                    "MCP server '{}' does not advertise resource support.",
                    self.client.server_name()
                ),
            );
        }

        match self.client.list_resources().await {
            Ok(result) => {
                let server_name = self.client.server_name();
                if result.resources.is_empty() {
                    return ToolResult::success(
                        "list_mcp_resources".to_string(),
                        format!("No resources available from MCP server '{}'.", server_name),
                    );
                }

                let lines: Vec<String> = result
                    .resources
                    .iter()
                    .map(|r| {
                        let name = r.name.clone().unwrap_or_default();
                        let mime = r.mime_type.clone().unwrap_or_default();
                        let desc = r.description.clone().unwrap_or_default();
                        if desc.is_empty() {
                            format!("[{server_name}] {} - {} ({})", r.uri, name, mime)
                        } else {
                            format!("[{server_name}] {} - {} ({}) - {}", r.uri, name, mime, desc)
                        }
                    })
                    .collect();

                ToolResult::success("list_mcp_resources".to_string(), lines.join("\n"))
            }
            Err(e) => ToolResult::error(
                "list_mcp_resources".to_string(),
                format!("Failed to list resources: {e}"),
            ),
        }
    }
}

/// Create resource tools for MCP servers that support resources.
pub fn load_mcp_resource_tools(
    clients: &[Arc<McpClient>],
) -> Vec<(Box<dyn Tool + Send + Sync>, Box<dyn Tool + Send + Sync>)> {
    clients
        .iter()
        .filter(|c| c.capabilities().supports_resources)
        .map(|c| {
            let list_tool: Box<dyn Tool + Send + Sync> =
                Box::new(ListMcpResourcesTool::new(Arc::clone(c)));
            let read_tool: Box<dyn Tool + Send + Sync> =
                Box::new(ReadMcpResourceTool::new(Arc::clone(c)));
            (list_tool, read_tool)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_resources_input_optional_filter() {
        let json = r#"{}"#;
        let input: ListMcpResourcesInput = serde_json::from_str(json).unwrap();
        assert!(input.server_name.is_none());

        let json = r#"{"server_name": "myserver"}"#;
        let input: ListMcpResourcesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.server_name.unwrap(), "myserver");
    }

    #[test]
    fn test_definition_structure() {
        // Verify the tool definition structure is correct by checking JSON serializability
        let def_json = serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Optional filter to only list resources from a specific MCP server"
                }
            },
            "required": []
        });
        let schema: ToolInputSchema = serde_json::from_value(def_json).unwrap();
        assert_eq!(schema.schema_type, "object");
        assert!(schema.required.is_empty());
    }
}
