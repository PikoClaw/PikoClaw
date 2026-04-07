// RemoteTriggerTool — cross-session event dispatch.

use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;

pub struct RemoteTriggerTool;

#[derive(Debug, Deserialize)]
struct RemoteTriggerInput {
    session_id: String,
    event_name: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[async_trait]
impl Tool for RemoteTriggerTool {
    fn name(&self) -> &'static str {
        "RemoteTrigger"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "RemoteTrigger".to_string(),
            description: "Send a named event to another active session. \
                Use this to coordinate across parallel sessions or notify a parent session of results."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "session_id": {
                        "type": "string",
                        "description": "The target session ID to trigger"
                    },
                    "event_name": {
                        "type": "string",
                        "description": "Name of the event to send (e.g., 'task_complete', 'result_ready')"
                    },
                    "payload": {
                        "type": "object",
                        "description": "Optional JSON payload to deliver with the event",
                        "additionalProperties": true
                    }
                }),
                required: vec!["session_id".to_string(), "event_name".to_string()],
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

        let params: RemoteTriggerInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {e}")),
        };

        let client = reqwest::Client::new();
        let url = format!(
            "https://api.claude.ai/api/sessions/{}/trigger",
            params.session_id
        );

        let body = serde_json::json!({
            "event_name": params.event_name,
            "payload": params.payload,
        });

        let resp = match client.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(id, format!("HTTP error: {e}")),
        };

        let target_prefix = &params.session_id[..params.session_id.len().min(8)];

        if resp.status().is_success() {
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    let delivered = data["delivered"].as_bool().unwrap_or(false);
                    let status = data["session_status"].as_str().unwrap_or("unknown");
                    ToolResult::success(
                        id,
                        format!(
                            "Event '{}' {} to session {} (status: {})",
                            params.event_name,
                            if delivered { "delivered" } else { "queued" },
                            target_prefix,
                            status,
                        ),
                    )
                }
                Err(_) => ToolResult::success(
                    id,
                    format!(
                        "Event '{}' sent to session {}",
                        params.event_name, target_prefix,
                    ),
                ),
            }
        } else {
            ToolResult::error(
                id,
                format!(
                    "Trigger failed: HTTP {} — is session {} active?",
                    resp.status(),
                    target_prefix,
                ),
            )
        }
    }
}
