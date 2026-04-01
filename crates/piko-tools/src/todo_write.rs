use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

pub type TodoStore = Arc<Mutex<HashMap<String, Vec<TodoItem>>>>;

pub struct TodoWriteTool {
    store: TodoStore,
}

impl TodoWriteTool {
    pub fn new(store: TodoStore) -> Self {
        Self { store }
    }

    pub fn with_shared_store() -> (Self, TodoStore) {
        let store: TodoStore = Arc::new(Mutex::new(HashMap::new()));
        (
            Self {
                store: Arc::clone(&store),
            },
            store,
        )
    }
}

#[derive(Debug, Deserialize)]
struct TodoWriteInput {
    todos: Vec<TodoItemInput>,
}

#[derive(Debug, Deserialize)]
struct TodoItemInput {
    id: Option<String>,
    content: String,
    status: Option<String>,
    priority: Option<String>,
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &'static str {
        "TodoWrite"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TodoWrite".to_string(),
            description: "Create and manage a structured task list for the current session. Use proactively to track progress on complex multi-step tasks. Mark tasks in_progress before starting, completed immediately after finishing. Only one task should be in_progress at a time.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "todos": {
                        "type": "array",
                        "description": "The updated todo list",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "Unique identifier for the todo item"
                                },
                                "content": {
                                    "type": "string",
                                    "description": "The task description (imperative form, e.g. 'Fix authentication bug')"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Current status of the task"
                                },
                                "priority": {
                                    "type": "string",
                                    "enum": ["high", "medium", "low"],
                                    "description": "Task priority"
                                }
                            },
                            "required": ["content", "status"]
                        }
                    }
                }),
                required: vec!["todos".to_string()],
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

        let parsed: TodoWriteInput = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(id, format!("invalid input: {}", e)),
        };

        let todos: Vec<TodoItem> = parsed
            .todos
            .into_iter()
            .enumerate()
            .map(|(i, t)| TodoItem {
                id: t.id.unwrap_or_else(|| format!("todo-{}", i + 1)),
                content: t.content,
                status: match t.status.as_deref() {
                    Some("in_progress") => TodoStatus::InProgress,
                    Some("completed") => TodoStatus::Completed,
                    _ => TodoStatus::Pending,
                },
                priority: t.priority,
            })
            .collect();

        let all_done = todos.iter().all(|t| t.status == TodoStatus::Completed);
        let stored = if all_done { vec![] } else { todos.clone() };

        let session_key = "current".to_string();
        {
            let mut store = self.store.lock().unwrap();
            store.insert(session_key, stored);
        }

        let summary = todos
            .iter()
            .map(|t| {
                let marker = match t.status {
                    TodoStatus::Completed => "✓",
                    TodoStatus::InProgress => "→",
                    TodoStatus::Pending => "○",
                };
                format!("{} {}", marker, t.content)
            })
            .collect::<Vec<_>>()
            .join("\n");

        ToolResult::success(
            id,
            format!(
                "Todos updated successfully. Ensure that you continue to use the todo list to track your progress.\n\n{}",
                summary
            ),
        )
    }
}
