use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct NotebookEditTool;

#[derive(Debug, Deserialize)]
struct NotebookEditInput {
    notebook_path: String,
    #[serde(default)]
    cell_id: Option<String>,
    new_source: String,
    #[serde(default)]
    cell_type: Option<String>,
    #[serde(default)]
    edit_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NotebookContent {
    nbformat: u32,
    #[serde(default)]
    nbformat_minor: u32,
    metadata: serde_json::Value,
    cells: Vec<NotebookCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotebookCell {
    cell_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    source: serde_json::Value,
    metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_count: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<serde_json::Value>>,
}

fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

#[allow(dead_code)]
fn source_to_string(source: &serde_json::Value) -> String {
    match source {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn string_to_source(s: &str) -> serde_json::Value {
    serde_json::Value::Array(
        s.split('\n')
            .enumerate()
            .map(|(i, line)| {
                let line_str = if i == 0 {
                    line.to_string()
                } else {
                    format!("\n{}", line)
                };
                serde_json::Value::String(line_str)
            })
            .collect(),
    )
}

fn random_cell_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:x}", t)
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &'static str {
        "NotebookEdit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "NotebookEdit".to_string(),
            description: "Replace, insert, or delete cells in a Jupyter notebook (.ipynb file). The notebook_path must be absolute. Use edit_mode=insert to add a new cell after cell_id (or at beginning). Use edit_mode=delete to remove a cell.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "notebook_path": {
                        "type": "string",
                        "description": "Absolute path to the Jupyter notebook file (.ipynb)"
                    },
                    "cell_id": {
                        "type": "string",
                        "description": "The ID of the cell to edit. For insert, new cell is placed after this cell."
                    },
                    "new_source": {
                        "type": "string",
                        "description": "The new source for the cell"
                    },
                    "cell_type": {
                        "type": "string",
                        "enum": ["code", "markdown"],
                        "description": "Cell type (required for insert mode)"
                    },
                    "edit_mode": {
                        "type": "string",
                        "enum": ["replace", "insert", "delete"],
                        "description": "Edit operation (default: replace)"
                    }
                }),
                required: vec!["notebook_path".to_string(), "new_source".to_string()],
            },
        }
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        let path = input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let mode = input
            .get("edit_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("replace");
        format!("{} cell in {}", mode, path)
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let parsed: NotebookEditInput = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(id, format!("invalid input: {}", e)),
        };

        let path = resolve_path(&ctx.cwd, &parsed.notebook_path);

        if path.extension().and_then(|e| e.to_str()) != Some("ipynb") {
            return ToolResult::error(
                id,
                "File must be a Jupyter notebook (.ipynb). For other files use Edit tool."
                    .to_string(),
            );
        }

        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::error(id, format!("failed to read {}: {}", path.display(), e))
            }
        };

        let mut notebook: NotebookContent = match serde_json::from_str(&content) {
            Ok(n) => n,
            Err(e) => return ToolResult::error(id, format!("invalid notebook JSON: {}", e)),
        };

        let edit_mode = parsed.edit_mode.as_deref().unwrap_or("replace");
        let cell_type = parsed.cell_type.as_deref().unwrap_or("code");

        let cell_index = if let Some(ref cid) = parsed.cell_id {
            let found = notebook
                .cells
                .iter()
                .position(|c| c.id.as_deref() == Some(cid.as_str()));
            if let Some(idx) = found {
                idx
            } else if let Ok(n) = cid.parse::<usize>() {
                if n < notebook.cells.len() {
                    n
                } else {
                    return ToolResult::error(id, format!("cell index {} out of bounds", n));
                }
            } else {
                return ToolResult::error(id, format!("cell '{}' not found", cid));
            }
        } else if edit_mode == "insert" {
            0
        } else {
            return ToolResult::error(id, "cell_id required when not inserting".to_string());
        };

        let supports_ids =
            notebook.nbformat > 4 || (notebook.nbformat == 4 && notebook.nbformat_minor >= 5);

        match edit_mode {
            "delete" => {
                notebook.cells.remove(cell_index);
                let updated = serde_json::to_string_pretty(&notebook).unwrap_or_default();
                if let Err(e) = fs::write(&path, &updated).await {
                    return ToolResult::error(
                        id,
                        format!("failed to write {}: {}", path.display(), e),
                    );
                }
                ToolResult::success(id, format!("deleted cell at index {}", cell_index))
            }
            "insert" => {
                let insert_at = if parsed.cell_id.is_some() {
                    cell_index + 1
                } else {
                    0
                };
                let new_id = if supports_ids {
                    Some(random_cell_id())
                } else {
                    None
                };
                let new_cell = if cell_type == "markdown" {
                    NotebookCell {
                        cell_type: "markdown".to_string(),
                        id: new_id.clone(),
                        source: string_to_source(&parsed.new_source),
                        metadata: serde_json::json!({}),
                        execution_count: None,
                        outputs: None,
                    }
                } else {
                    NotebookCell {
                        cell_type: "code".to_string(),
                        id: new_id.clone(),
                        source: string_to_source(&parsed.new_source),
                        metadata: serde_json::json!({}),
                        execution_count: Some(serde_json::Value::Null),
                        outputs: Some(vec![]),
                    }
                };
                notebook.cells.insert(insert_at, new_cell);
                let updated = serde_json::to_string_pretty(&notebook).unwrap_or_default();
                if let Err(e) = fs::write(&path, &updated).await {
                    return ToolResult::error(
                        id,
                        format!("failed to write {}: {}", path.display(), e),
                    );
                }
                ToolResult::success(
                    id,
                    format!("inserted {} cell at index {}", cell_type, insert_at),
                )
            }
            _ => {
                let cell = &mut notebook.cells[cell_index];
                cell.source = string_to_source(&parsed.new_source);
                if cell.cell_type == "code" {
                    cell.execution_count = Some(serde_json::Value::Null);
                    cell.outputs = Some(vec![]);
                }
                let cid = cell.id.clone().unwrap_or_else(|| cell_index.to_string());
                let updated = serde_json::to_string_pretty(&notebook).unwrap_or_default();
                if let Err(e) = fs::write(&path, &updated).await {
                    return ToolResult::error(
                        id,
                        format!("failed to write {}: {}", path.display(), e),
                    );
                }
                ToolResult::success(id, format!("updated cell {}", cid))
            }
        }
    }
}
