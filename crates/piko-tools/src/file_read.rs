use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct FileReadTool;

#[derive(Debug, Deserialize)]
struct FileReadInput {
    file_path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &'static str {
        "Read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Read".to_string(),
            description: "Reads a file from the filesystem. Returns file contents with line numbers. Supports offset and limit for large files.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-indexed)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                }),
                required: vec!["file_path".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let parsed: FileReadInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        let tool_use_id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let path = resolve_path(&ctx.cwd, &parsed.file_path);

        match fs::read_to_string(&path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = parsed.offset.unwrap_or(1).saturating_sub(1);
                let end = parsed
                    .limit
                    .map(|l| (start + l).min(lines.len()))
                    .unwrap_or(lines.len());

                let numbered: String = lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n");

                ToolResult::success(tool_use_id, numbered)
            }
            Err(e) => ToolResult::error(
                tool_use_id,
                format!("failed to read {}: {}", path.display(), e),
            ),
        }
    }

    fn is_read_only(&self) -> bool {
        true
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
