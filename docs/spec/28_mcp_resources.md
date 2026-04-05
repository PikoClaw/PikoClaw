# Spec: MCP Resource Reading

**Status**: ✅ Done — `ListMcpResourcesTool` and `ReadMcpResourceTool` implemented; protocol types, client extension, and tool registration complete
**Rust crate**: `piko-mcp` (`mcp_list_resources.rs`, `mcp_read_resource.rs`, `protocol.rs`), `piko-tools`
**TS source**: `tools/ListMcpResourcesTool.tsx`, `tools/ReadMcpResourceTool.tsx`

---

## Overview

MCP servers can expose not just tools but also **resources** — files, data streams, API endpoints, or any URI-addressable content. The agent can list available resources and read their content.

This is distinct from tools: resources are passive data sources, tools are active operations.

---

## MCP Protocol: Resources

### List Resources

```json
// Request
{ "jsonrpc": "2.0", "id": 1, "method": "resources/list", "params": {} }

// Response
{
  "resources": [
    {
      "uri": "file:///path/to/file.txt",
      "name": "Important Config",
      "description": "Main configuration file",
      "mimeType": "text/plain"
    },
    {
      "uri": "https://api.example.com/data",
      "name": "Live API Data",
      "mimeType": "application/json"
    }
  ]
}
```

### Read Resource

```json
// Request
{
  "jsonrpc": "2.0", "id": 2,
  "method": "resources/read",
  "params": { "uri": "file:///path/to/file.txt" }
}

// Response
{
  "contents": [
    {
      "uri": "file:///path/to/file.txt",
      "mimeType": "text/plain",
      "text": "file content here..."
    }
  ]
}
// or for binary:
{
  "contents": [
    {
      "uri": "...",
      "mimeType": "image/png",
      "blob": "<base64-encoded>"
    }
  ]
}
```

### Resource Subscriptions (Optional)

```json
// Subscribe to changes
{ "method": "resources/subscribe", "params": { "uri": "..." } }

// Server pushes notification when resource changes:
{ "method": "notifications/resources/updated", "params": { "uri": "..." } }
```

---

## Implementation Plan

### Step 1: Add Resource Methods to MCP Client (`piko-mcp`)

```rust
// In piko-mcp/client.rs
impl McpClient {
    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        let response = self.request("resources/list", json!({})).await?;
        Ok(serde_json::from_value(response["resources"].clone())?)
    }

    pub async fn read_resource(&self, uri: &str) -> Result<Vec<McpResourceContent>> {
        let response = self.request("resources/read", json!({ "uri": uri })).await?;
        Ok(serde_json::from_value(response["contents"].clone())?)
    }
}

pub struct McpResource {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

pub struct McpResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,   // for text resources
    pub blob: Option<String>,   // for binary (base64)
}
```

### Step 2: ListMcpResourcesTool (`piko-tools`)

```rust
pub struct ListMcpResourcesTool {
    mcp_clients: Arc<HashMap<String, McpClient>>,
}

impl Tool for ListMcpResourcesTool {
    fn name() -> &'static str { "list_mcp_resources" }
    fn description() -> &'static str {
        "List all resources available from connected MCP servers."
    }
    // Input: { server_name?: string }
    async fn execute(&self, input: ListMcpResourcesInput, _: &ToolContext) -> ToolResult {
        let mut all_resources = vec![];
        for (server_name, client) in &*self.mcp_clients {
            if let Some(filter) = &input.server_name {
                if server_name != filter { continue; }
            }
            if let Ok(resources) = client.list_resources().await {
                for r in resources {
                    all_resources.push(format!(
                        "[{}] {} - {} ({})",
                        server_name,
                        r.uri,
                        r.name.unwrap_or_default(),
                        r.mime_type.unwrap_or_default()
                    ));
                }
            }
        }
        ToolResult::success(all_resources.join("\n"))
    }
}
```

### Step 3: ReadMcpResourceTool (`piko-tools`)

```rust
pub struct ReadMcpResourceTool {
    mcp_clients: Arc<HashMap<String, McpClient>>,
}

// Input: { server_name: string, uri: string }
async fn execute(&self, input: ReadMcpResourceInput, _: &ToolContext) -> ToolResult {
    let client = self.mcp_clients.get(&input.server_name)
        .ok_or_else(|| format!("MCP server '{}' not found", input.server_name))?;

    let contents = client.read_resource(&input.uri).await?;

    let text = contents.iter()
        .filter_map(|c| c.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n");

    ToolResult::success(text)
}
```

### Step 4: Capability Check

Before calling `resources/list`, check if the server advertised resources capability during `initialize`:

```rust
// In McpClient, store server capabilities from initialize response
pub struct ServerCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
}
```

Only expose `ListMcpResourcesTool` / `ReadMcpResourceTool` if at least one connected server supports resources.

---

## Priority

Medium. Some popular MCP servers (filesystem, database connectors) expose resources. Completing this rounds out MCP support to full protocol parity.
