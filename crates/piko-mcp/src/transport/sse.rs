use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

pub struct SseTransport {
    client: Client,
    base_url: String,
    session_id: Mutex<Option<String>>,
    next_id: AtomicU64,
}

impl SseTransport {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = Client::new();
        let transport = Self {
            client,
            base_url: url.trim_end_matches('/').to_string(),
            session_id: Mutex::new(None),
            next_id: AtomicU64::new(1),
        };
        Ok(transport)
    }

    pub async fn send(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let session = self.session_id.lock().await.clone();
        let mut req = self
            .client
            .post(format!("{}/message", self.base_url))
            .json(request);

        if let Some(ref sid) = session {
            req = req.header("mcp-session-id", sid);
        }

        let resp = req.send().await?;

        if let Some(sid) = resp.headers().get("mcp-session-id") {
            *self.session_id.lock().await = Some(sid.to_str()?.to_string());
        }

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("MCP SSE server error {}: {}", status, body));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if content_type.contains("text/event-stream") {
            let mut stream = resp.bytes_stream();
            let mut buf = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buf.push_str(&String::from_utf8_lossy(&chunk));
                for line in buf.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(data) {
                            return Ok(response);
                        }
                    }
                }
            }
            Err(anyhow!("SSE stream ended without response"))
        } else {
            let response: JsonRpcResponse = resp.json().await?;
            Ok(response)
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}
