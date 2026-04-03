use crate::protocol::{
    JsonRpcRequest, JsonRpcResponse, McpCallToolResult, McpListResourcesResult, McpListToolsResult,
    McpReadResourceResult, McpServerCapabilities,
};
use crate::server_config::{McpServerConfig, McpTransportConfig};
use crate::transport::{SseTransport, StdioTransport};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

enum Transport {
    Stdio(Mutex<StdioTransport>),
    Sse(SseTransport),
}

pub struct McpClient {
    name: String,
    transport: Transport,
    next_id: AtomicU64,
    capabilities: std::sync::Mutex<McpServerCapabilities>,
}

impl McpClient {
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        match &config.transport {
            McpTransportConfig::Stdio { command, args, env } => {
                let transport = StdioTransport::spawn(command, args, env.as_ref()).await?;
                let client = Self {
                    name: config.name.clone(),
                    transport: Transport::Stdio(Mutex::new(transport)),
                    next_id: AtomicU64::new(1),
                    capabilities: std::sync::Mutex::new(McpServerCapabilities::default()),
                };
                client.initialize().await?;
                Ok(client)
            }
            McpTransportConfig::Sse { url } => {
                let transport = SseTransport::connect(url).await?;
                let client = Self {
                    name: config.name.clone(),
                    transport: Transport::Sse(transport),
                    next_id: AtomicU64::new(1),
                    capabilities: std::sync::Mutex::new(McpServerCapabilities::default()),
                };
                client.initialize().await?;
                Ok(client)
            }
        }
    }

    async fn send(&self, req: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        match &self.transport {
            Transport::Stdio(m) => m.lock().await.send(req).await,
            Transport::Sse(sse) => sse.send(req).await,
        }
    }

    async fn initialize(&self) -> Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(
            id,
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "clientInfo": { "name": "pikoclaw", "version": "0.1.0" }
            })),
        );
        let resp = self.send(&req).await?;

        if let Some(result) = &resp.result {
            let mut caps = self.capabilities.lock().unwrap();
            if let Some(capabilities) = result.get("capabilities") {
                if let Some(tools) = capabilities.get("tools") {
                    caps.supports_tools = !tools.is_null();
                }
                if let Some(resources) = capabilities.get("resources") {
                    caps.supports_resources = !resources.is_null();
                }
            }
        }

        Ok(())
    }

    pub fn capabilities(&self) -> McpServerCapabilities {
        self.capabilities.lock().unwrap().clone()
    }

    /// List resources available from this MCP server.
    /// Only available if the server advertised resource support during initialize.
    pub async fn list_resources(&self) -> Result<McpListResourcesResult> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(id, "resources/list", None);
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(anyhow!("MCP error {}: {}", err.code, err.message));
        }

        let result: McpListResourcesResult =
            serde_json::from_value(resp.result.unwrap_or(Value::Object(Default::default())))?;
        Ok(result)
    }

    /// Read a resource by URI from this MCP server.
    pub async fn read_resource(&self, uri: &str) -> Result<McpReadResourceResult> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(
            id,
            "resources/read",
            Some(serde_json::json!({ "uri": uri })),
        );
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(anyhow!("MCP error {}: {}", err.code, err.message));
        }

        let result: McpReadResourceResult =
            serde_json::from_value(resp.result.unwrap_or(Value::Object(Default::default())))?;
        Ok(result)
    }

    pub async fn list_tools(&self) -> Result<McpListToolsResult> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(id, "tools/list", None);
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(anyhow!("MCP error {}: {}", err.code, err.message));
        }

        let result: McpListToolsResult =
            serde_json::from_value(resp.result.unwrap_or(Value::Object(Default::default())))?;
        Ok(result)
    }

    pub async fn call_tool(&self, name: &str, input: Value) -> Result<McpCallToolResult> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(
            id,
            "tools/call",
            Some(serde_json::json!({ "name": name, "arguments": input })),
        );
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(anyhow!("MCP error {}: {}", err.code, err.message));
        }

        let result: McpCallToolResult =
            serde_json::from_value(resp.result.unwrap_or(Value::Object(Default::default())))?;
        Ok(result)
    }

    pub fn server_name(&self) -> &str {
        &self.name
    }
}
