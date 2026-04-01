use crate::protocol::{JsonRpcRequest, JsonRpcResponse, McpCallToolResult, McpListToolsResult};
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
                "capabilities": {},
                "clientInfo": { "name": "pikoclaw", "version": "0.1.0" }
            })),
        );
        self.send(&req).await?;
        Ok(())
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
