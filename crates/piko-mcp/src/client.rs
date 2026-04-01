use crate::protocol::{JsonRpcRequest, McpCallToolResult, McpListToolsResult};
use crate::server_config::{McpServerConfig, McpTransportConfig};
use crate::transport::StdioTransport;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

pub struct McpClient {
    name: String,
    transport: Mutex<StdioTransport>,
    next_id: AtomicU64,
}

impl McpClient {
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        match &config.transport {
            McpTransportConfig::Stdio { command, args, .. } => {
                let transport = StdioTransport::spawn(command, args).await?;
                let client = Self {
                    name: config.name.clone(),
                    transport: Mutex::new(transport),
                    next_id: AtomicU64::new(1),
                };
                client.initialize().await?;
                Ok(client)
            }
            McpTransportConfig::Sse { .. } => Err(anyhow!("SSE transport not yet implemented")),
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
        let mut transport = self.transport.lock().await;
        transport.send(&req).await?;
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<McpListToolsResult> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(id, "tools/list", None);
        let mut transport = self.transport.lock().await;
        let resp = transport.send(&req).await?;

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
        let mut transport = self.transport.lock().await;
        let resp = transport.send(&req).await?;

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
