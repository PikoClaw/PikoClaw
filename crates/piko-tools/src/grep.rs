use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use ignore::WalkBuilder;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use regex::RegexBuilder;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub struct GrepTool;

#[derive(Debug, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default = "default_context")]
    context: usize,
}

fn default_context() -> usize {
    0
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "Grep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Grep".to_string(),
            description: "Searches file contents using regex. Returns matching lines with file path and line number. Respects .gitignore.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "pattern": {
                        "type": "string",
                        "description": "Regular expression pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search (defaults to cwd)"
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g. '*.rs', '*.{ts,tsx}')"
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "description": "Case-insensitive search (default: false)"
                    },
                    "context": {
                        "type": "integer",
                        "description": "Lines of context before and after each match (default: 0)"
                    }
                }),
                required: vec!["pattern".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let parsed: GrepInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        let tool_use_id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let regex = match RegexBuilder::new(&parsed.pattern)
            .case_insensitive(parsed.case_insensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(tool_use_id, format!("invalid regex pattern: {}", e))
            }
        };

        let search_path = match &parsed.path {
            Some(p) => resolve_path(&ctx.cwd, p),
            None => ctx.cwd.clone(),
        };

        let glob_override = parsed.glob.as_deref();

        let mut results: Vec<String> = Vec::new();
        let mut match_count = 0;
        const MAX_MATCHES: usize = 500;

        let mut walker_builder = WalkBuilder::new(&search_path);
        walker_builder.hidden(false).ignore(true).git_ignore(true);
        if let Some(glob) = glob_override {
            let mut override_builder = ignore::overrides::OverrideBuilder::new(&search_path);
            let _ = override_builder.add(glob);
            if let Ok(overrides) = override_builder.build() {
                walker_builder.overrides(overrides);
            }
        }

        for entry in walker_builder.build() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let file_path = entry.path();
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut matched_lines: Vec<usize> = Vec::new();

            for (i, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    matched_lines.push(i);
                }
            }

            if matched_lines.is_empty() {
                continue;
            }

            let mut shown: std::collections::HashSet<usize> = std::collections::HashSet::new();
            let mut file_results: Vec<String> = Vec::new();

            for &line_idx in &matched_lines {
                if match_count >= MAX_MATCHES {
                    break;
                }
                let start = line_idx.saturating_sub(parsed.context);
                let end = (line_idx + parsed.context + 1).min(lines.len());

                for (i, line) in lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(j, l)| (start + j, l))
                {
                    if shown.insert(i) {
                        let prefix = if i == line_idx { ">" } else { " " };
                        file_results.push(format!(
                            "{}{}:{}: {}",
                            prefix,
                            file_path.display(),
                            i + 1,
                            line
                        ));
                        if i == line_idx {
                            match_count += 1;
                        }
                    }
                }
            }

            results.extend(file_results);
        }

        if results.is_empty() {
            ToolResult::success(tool_use_id, "No matches found".to_string())
        } else {
            let mut output = results.join("\n");
            if match_count >= MAX_MATCHES {
                output.push_str(&format!("\n... (truncated at {} matches)", MAX_MATCHES));
            }
            ToolResult::success(tool_use_id, output)
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
