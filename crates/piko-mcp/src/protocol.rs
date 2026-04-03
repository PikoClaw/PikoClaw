use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(id.into())),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpListToolsResult {
    pub tools: Vec<McpToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallToolResult {
    pub content: Vec<McpContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContent {
    Text { text: String },
    Image { data: String, mime_type: String },
}

// ── MCP Resource types (spec 28) ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpListResourcesResult {
    pub resources: Vec<McpResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpReadResourceResult {
    pub contents: Vec<McpResourceContent>,
}

/// Server capabilities discovered during initialize
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerCapabilities {
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_resources: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = McpResource {
            uri: "file:///etc/hosts".to_string(),
            name: Some("Hosts File".to_string()),
            description: Some("System hosts configuration".to_string()),
            mime_type: Some("text/plain".to_string()),
        };
        let json = serde_json::to_string(&resource).unwrap();
        assert!(json.contains("file:///etc/hosts"));
        assert!(json.contains("Hosts File"));
        assert!(json.contains("mimeType"));
    }

    #[test]
    fn test_mcp_resource_deserialization() {
        let json = r#"{"uri":"file:///test.txt","name":"Test","mimeType":"text/plain"}"#;
        let resource: McpResource = serde_json::from_str(json).unwrap();
        assert_eq!(resource.uri, "file:///test.txt");
        assert_eq!(resource.name.unwrap(), "Test");
        assert_eq!(resource.mime_type.unwrap(), "text/plain");
        assert!(resource.description.is_none());
    }

    #[test]
    fn test_mcp_list_resources_result() {
        let json = r#"{
            "resources": [
                {"uri": "file:///a.txt", "name": "File A"},
                {"uri": "file:///b.txt", "name": "File B", "description": "Some file"}
            ]
        }"#;
        let result: McpListResourcesResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.resources.len(), 2);
        assert_eq!(result.resources[0].uri, "file:///a.txt");
        assert_eq!(result.resources[1].name.as_ref().unwrap(), "File B");
    }

    #[test]
    fn test_mcp_resource_content_serialization() {
        let content = McpResourceContent {
            uri: "file:///config.yaml".to_string(),
            mime_type: Some("text/yaml".to_string()),
            text: Some("key: value".to_string()),
            blob: None,
        };
        let json = serde_json::to_string(&content).unwrap();
        let parsed: McpResourceContent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uri, "file:///config.yaml");
        assert_eq!(parsed.text.unwrap(), "key: value");
    }

    #[test]
    fn test_mcp_resource_content_with_blob() {
        let content = McpResourceContent {
            uri: "file:///image.png".to_string(),
            mime_type: Some("image/png".to_string()),
            text: None,
            blob: Some("iVBORw0KGgo=".to_string()),
        };
        assert!(content.text.is_none());
        assert!(content.blob.is_some());
    }

    #[test]
    fn test_mcp_read_resource_result() {
        let json = r#"{
            "contents": [
                {"uri": "file:///test.txt", "text": "hello world", "mimeType": "text/plain"}
            ]
        }"#;
        let result: McpReadResourceResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0].text.as_ref().unwrap(), "hello world");
    }

    #[test]
    fn test_mcp_server_capabilities_default() {
        let caps = McpServerCapabilities::default();
        assert!(!caps.supports_tools);
        assert!(!caps.supports_resources);
    }

    #[test]
    fn test_mcp_server_capabilities_deserialization() {
        let json = r#"{"supports_tools": true, "supports_resources": true}"#;
        let caps: McpServerCapabilities = serde_json::from_str(json).unwrap();
        assert!(caps.supports_tools);
        assert!(caps.supports_resources);
    }
}
