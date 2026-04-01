use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub struct GlobTool;

#[derive(Debug, Deserialize)]
struct GlobInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str {
        "Glob"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Glob".to_string(),
            description: "Finds files matching a glob pattern. Respects .gitignore. Returns matching file paths sorted by modification time.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match files (e.g. '**/*.rs', 'src/**/*.ts')"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (defaults to current working directory)"
                    }
                }),
                required: vec!["pattern".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let parsed: GlobInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        let tool_use_id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let search_dir = match &parsed.path {
            Some(p) => resolve_path(&ctx.cwd, p),
            None => ctx.cwd.clone(),
        };

        let glob = match Glob::new(&parsed.pattern) {
            Ok(g) => g,
            Err(e) => {
                return ToolResult::error(tool_use_id, format!("invalid glob pattern: {}", e))
            }
        };

        let mut builder = GlobSetBuilder::new();
        builder.add(glob);
        let glob_set = match builder.build() {
            Ok(g) => g,
            Err(e) => {
                return ToolResult::error(tool_use_id, format!("failed to build glob: {}", e))
            }
        };

        let mut matches: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();

        let walker = WalkBuilder::new(&search_dir)
            .hidden(false)
            .ignore(true)
            .git_ignore(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let path = entry.path();
                let relative = path.strip_prefix(&search_dir).unwrap_or(path);
                if glob_set.is_match(relative) || glob_set.is_match(path) {
                    let mtime = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    matches.push((mtime, path.to_path_buf()));
                }
            }
        }

        matches.sort_by(|a, b| b.0.cmp(&a.0));

        let paths: Vec<String> = matches
            .iter()
            .map(|(_, p)| p.display().to_string())
            .collect();

        if paths.is_empty() {
            ToolResult::success(tool_use_id, "No files found matching pattern".to_string())
        } else {
            ToolResult::success(tool_use_id, paths.join("\n"))
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
