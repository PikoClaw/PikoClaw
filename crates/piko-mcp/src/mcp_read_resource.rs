use async_trait::async_trait;
use piko_tools::tool_trait::{Tool, ToolContext};
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::sync::Arc;

use crate::client::McpClient;

#[derive(Debug, Deserialize)]
pub struct ReadMcpResourceInput {
    pub server_name: String,
    pub uri: String,
}

pub struct ReadMcpResourceTool {
    client: Arc<McpClient>,
}

impl ReadMcpResourceTool {
    pub fn new(client: Arc<McpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &'static str {
        "read_mcp_resource"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: "Read a specific resource by URI from a connected MCP server. Use list_mcp_resources first to discover available resources.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "server_name": {
                        "type": "string",
                        "description": "Name of the MCP server to read the resource from"
                    },
                    "uri": {
                        "type": "string",
                        "description": "URI of the resource to read"
                    }
                }),
                required: vec!["server_name".to_string(), "uri".to_string()],
            },
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let input: ReadMcpResourceInput = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::error(
                    "read_mcp_resource".to_string(),
                    format!("invalid input: {e}"),
                );
            }
        };

        let caps = self.client.capabilities();
        if !caps.supports_resources {
            return ToolResult::success(
                "read_mcp_resource".to_string(),
                format!(
                    "MCP server '{}' does not advertise resource support.",
                    self.client.server_name()
                ),
            );
        }

        match self.client.read_resource(&input.uri).await {
            Ok(result) => {
                let text: Vec<&str> = result
                    .contents
                    .iter()
                    .filter_map(|c| c.text.as_deref().or(c.blob.as_deref()))
                    .collect();

                if text.is_empty() {
                    ToolResult::success(
                        "read_mcp_resource".to_string(),
                        format!(
                            "Resource '{}' from server '{}' returned empty content.",
                            input.uri,
                            self.client.server_name()
                        ),
                    )
                } else {
                    ToolResult::success("read_mcp_resource".to_string(), text.join("\n"))
                }
            }
            Err(e) => ToolResult::error(
                "read_mcp_resource".to_string(),
                format!(
                    "Failed to read resource '{}' from server '{}': {}",
                    input.uri,
                    self.client.server_name(),
                    e
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_resource_input_required_fields() {
        let json = r#"{"server_name": "test", "uri": "file:///test.txt"}"#;
        let input: ReadMcpResourceInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.server_name, "test");
        assert_eq!(input.uri, "file:///test.txt");
    }

    #[test]
    fn test_read_resource_input_missing_fields() {
        let json = r#"{"server_name": "test"}"#;
        let result: Result<ReadMcpResourceInput, _> = serde_json::from_str(json);
        assert!(result.is_err());

        let json = r#"{"uri": "file:///test.txt"}"#;
        let result: Result<ReadMcpResourceInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_definition_structure() {
        let def_json = serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": { "type": "string", "description": "Name of the MCP server" },
                "uri": { "type": "string", "description": "URI of the resource" }
            },
            "required": ["server_name", "uri"]
        });
        let schema: ToolInputSchema = serde_json::from_value(def_json).unwrap();
        assert_eq!(schema.schema_type, "object");
        assert_eq!(schema.required.len(), 2);
        assert!(schema.required.contains(&"server_name".to_string()));
        assert!(schema.required.contains(&"uri".to_string()));
    }
}
