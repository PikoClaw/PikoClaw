use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct FileWriteTool;

#[derive(Debug, Deserialize)]
struct FileWriteInput {
    file_path: String,
    content: String,
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &'static str {
        "Write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Write".to_string(),
            description: "Writes content to a file, creating it or overwriting it. Creates parent directories as needed.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to write to"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                }),
                required: vec!["file_path".to_string(), "content".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let parsed: FileWriteInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        let tool_use_id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let path = resolve_path(&ctx.cwd, &parsed.file_path);

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                return ToolResult::error(
                    tool_use_id,
                    format!("failed to create directories: {}", e),
                );
            }
        }

        match fs::write(&path, &parsed.content).await {
            Ok(_) => ToolResult::success(
                tool_use_id,
                format!("wrote {} bytes to {}", parsed.content.len(), path.display()),
            ),
            Err(e) => ToolResult::error(
                tool_use_id,
                format!("failed to write {}: {}", path.display(), e),
            ),
        }
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        let path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        format!("write to file: {}", path)
    }
}

fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        cwd.join(p)
    }
}
