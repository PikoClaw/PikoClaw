use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

pub struct BriefTool;

#[derive(Debug, Deserialize)]
struct BriefInput {
    message: String,
    #[serde(default)]
    attachments: Vec<String>,
    #[serde(default = "default_status")]
    status: String,
}

fn default_status() -> String {
    "normal".to_string()
}

#[derive(Debug, Serialize)]
struct AttachmentMeta {
    path: String,
    size: u64,
    is_image: bool,
}

fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        cwd.join(p)
    }
}

#[async_trait]
impl Tool for BriefTool {
    fn name(&self) -> &'static str {
        "Brief"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Brief".to_string(),
            description: "Send a formatted message to the user, optionally with file attachments. \
                Use status=\"proactive\" when surfacing something the user hasn't asked for \
                (task completion, a blocker, an unsolicited update). \
                Use status=\"normal\" when replying to something the user just said."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "message": {
                        "type": "string",
                        "description": "The message to send. Supports Markdown."
                    },
                    "attachments": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional file paths to attach (images, diffs, logs)"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["normal", "proactive"],
                        "description": "Use 'proactive' for unsolicited updates, 'normal' for direct replies"
                    }
                }),
                required: vec!["message".to_string(), "status".to_string()],
            },
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let params: BriefInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        if params.message.trim().is_empty() {
            return ToolResult::error(id, "Message cannot be empty.");
        }

        let mut resolved: Vec<AttachmentMeta> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for raw_path in &params.attachments {
            let path = resolve_path(&ctx.cwd, raw_path);
            match resolve_attachment(&path).await {
                Ok(meta) => resolved.push(meta),
                Err(e) => errors.push(format!("{}: {}", raw_path, e)),
            }
        }

        if !errors.is_empty() {
            return ToolResult::error(
                id,
                format!("Failed to resolve attachments:\n{}", errors.join("\n")),
            );
        }

        debug!(
            status = %params.status,
            attachments = resolved.len(),
            "Brief message"
        );

        ToolResult::success(id, &params.message)
    }
}

async fn resolve_attachment(path: &Path) -> Result<AttachmentMeta, String> {
    let meta = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;

    if !meta.is_file() {
        return Err("not a file".to_string());
    }

    let size = meta.len();
    let is_image = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
            )
        })
        .unwrap_or(false);

    Ok(AttachmentMeta {
        path: path.display().to_string(),
        size,
        is_image,
    })
}
