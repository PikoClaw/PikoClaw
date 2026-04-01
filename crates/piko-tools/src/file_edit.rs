use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::fs;

pub struct FileEditTool;

#[derive(Debug, Deserialize)]
struct FileEditInput {
    file_path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &'static str {
        "Edit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Edit".to_string(),
            description: "Performs exact string replacement in a file. The old_string must match exactly (including whitespace). Use replace_all to replace every occurrence.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact text to find and replace"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false, replaces only first)",
                        "default": false
                    }
                }),
                required: vec!["file_path".to_string(), "old_string".to_string(), "new_string".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let parsed: FileEditInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        let tool_use_id = input.get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let path = resolve_path(&ctx.cwd, &parsed.file_path);

        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(tool_use_id, format!("failed to read {}: {}", path.display(), e)),
        };

        let count = content.matches(&parsed.old_string).count();
        if count == 0 {
            return ToolResult::error(
                tool_use_id,
                format!("old_string not found in {}", path.display()),
            );
        }

        if !parsed.replace_all && count > 1 {
            return ToolResult::error(
                tool_use_id,
                format!(
                    "old_string appears {} times in {}; use replace_all=true or provide more context to make it unique",
                    count,
                    path.display()
                ),
            );
        }

        let new_content = if parsed.replace_all {
            content.replace(&parsed.old_string, &parsed.new_string)
        } else {
            content.replacen(&parsed.old_string, &parsed.new_string, 1)
        };

        match fs::write(&path, &new_content).await {
            Ok(_) => ToolResult::success(
                tool_use_id,
                format!("replaced {} occurrence(s) in {}", count, path.display()),
            ),
            Err(e) => ToolResult::error(tool_use_id, format!("failed to write {}: {}", path.display(), e)),
        }
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        let path = input.get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        format!("edit file: {}", path)
    }
}

fn resolve_path(cwd: &PathBuf, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() { p } else { cwd.join(p) }
}
