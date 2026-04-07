// SendMessageTool: send a message to another agent or broadcast to all.
//
// In-process inbox backed by a global RwLock<HashMap> that works for
// sub-agents spawned within the same process.
//
// Messages are stored keyed by recipient name. Other agents can check
// their inbox by calling drain_inbox() or peek_inbox().

use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// In-process inbox
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    pub timestamp: u64,
}

type InboxMap = Arc<RwLock<HashMap<String, Vec<AgentMessage>>>>;

static INBOX: OnceLock<InboxMap> = OnceLock::new();

fn inbox() -> &'static InboxMap {
    INBOX.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Remove and return all messages queued for `recipient`.
pub async fn drain_inbox(recipient: &str) -> Vec<AgentMessage> {
    let mut map = inbox().write().await;
    map.remove(recipient).unwrap_or_default()
}

/// Read (without removing) all messages queued for `recipient`.
pub async fn peek_inbox(recipient: &str) -> Vec<AgentMessage> {
    let map = inbox().read().await;
    map.get(recipient).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

pub struct SendMessageTool;

#[derive(Debug, Deserialize)]
struct SendMessageInput {
    to: String,
    message: String,
    #[serde(default)]
    summary: Option<String>,
}

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &'static str {
        "SendMessage"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "SendMessage".to_string(),
            description: "Send a message to another agent by name, or broadcast to all active \
                agents with to=\"*\". Recipients accumulate messages in their inbox and can \
                retrieve them. Use this for coordination between concurrent sub-agents."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "to": {
                        "type": "string",
                        "description": "Recipient agent name or session ID. Use \"*\" to broadcast to all."
                    },
                    "message": {
                        "type": "string",
                        "description": "Message content"
                    },
                    "summary": {
                        "type": "string",
                        "description": "5–10 word preview for the UI (optional)"
                    }
                }),
                required: vec!["to".to_string(), "message".to_string()],
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

        let params: SendMessageInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        if params.message.is_empty() {
            return ToolResult::error(id, "Message cannot be empty.");
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let msg = AgentMessage {
            from: "agent".to_string(),
            to: params.to.clone(),
            content: params.message.clone(),
            timestamp: now,
        };

        let preview = params.summary.as_deref().unwrap_or_else(|| {
            let s = params.message.as_str();
            &s[..s.len().min(60)]
        });

        if params.to == "*" {
            let mut map = inbox().write().await;
            let recipients: Vec<String> = map.keys().cloned().collect();

            if recipients.is_empty() {
                return ToolResult::success(
                    id,
                    "Broadcast queued (no active recipient inboxes yet).",
                );
            }

            for key in &recipients {
                map.entry(key.clone()).or_default().push(msg.clone());
            }

            return ToolResult::success(
                id,
                format!("Broadcast to {} agent(s): {}", recipients.len(), preview),
            );
        }

        inbox()
            .write()
            .await
            .entry(params.to.clone())
            .or_default()
            .push(msg);

        ToolResult::success(id, format!("Message sent to '{}': {}", params.to, preview))
    }
}
