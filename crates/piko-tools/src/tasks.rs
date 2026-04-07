// Task management tools: TaskCreate, TaskGet, TaskUpdate, TaskList, TaskStop, TaskOutput.
//
// In-process task store backed by a global RwLock<HashMap>.
// Tasks have id, subject, description, status, owner, blocks/blocked-by dependencies,
// and optional output.

use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Task store (global singleton)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
    Running,
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Deleted => "deleted",
            TaskStatus::Running => "running",
            TaskStatus::Failed => "failed",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub blocks: Vec<String>,
    pub blocked_by: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub output: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Task {
    fn new(subject: impl Into<String>, description: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            subject: subject.into(),
            description: description.into(),
            status: TaskStatus::Pending,
            owner: None,
            blocks: vec![],
            blocked_by: vec![],
            metadata: None,
            output: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn to_summary_value(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "subject": self.subject,
            "status": self.status.to_string(),
            "owner": self.owner,
            "blocked_by": self.blocked_by,
        })
    }

    fn to_full_value(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "subject": self.subject,
            "description": self.description,
            "status": self.status.to_string(),
            "owner": self.owner,
            "blocks": self.blocks,
            "blocked_by": self.blocked_by,
            "metadata": self.metadata,
            "output": self.output,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
        })
    }
}

static TASK_STORE: OnceLock<Arc<RwLock<HashMap<String, Task>>>> = OnceLock::new();

fn task_store() -> &'static Arc<RwLock<HashMap<String, Task>>> {
    TASK_STORE.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

// ---------------------------------------------------------------------------
// TaskCreate
// ---------------------------------------------------------------------------

pub struct TaskCreateTool;

#[derive(Debug, Deserialize)]
struct TaskCreateInput {
    subject: String,
    description: String,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &'static str {
        "TaskCreate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TaskCreate".to_string(),
            description: "Create a new task to track work items. Returns the task ID.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "subject": { "type": "string", "description": "Brief title for the task" },
                    "description": { "type": "string", "description": "Detailed description of what needs to be done" },
                    "metadata": { "type": "object", "description": "Optional arbitrary metadata" }
                }),
                required: vec!["subject".to_string(), "description".to_string()],
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

        let params: TaskCreateInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let mut task = Task::new(&params.subject, &params.description);
        task.metadata = params.metadata;
        let task_id = task.id.clone();

        debug!(task_id = %task_id, subject = %params.subject, "Creating task");
        task_store().write().await.insert(task_id.clone(), task);

        ToolResult::success(
            id,
            serde_json::to_string_pretty(&serde_json::json!({
                "task_id": task_id,
                "subject": params.subject,
            }))
            .unwrap_or_default(),
        )
    }
}

// ---------------------------------------------------------------------------
// TaskGet
// ---------------------------------------------------------------------------

pub struct TaskGetTool;

#[derive(Debug, Deserialize)]
struct TaskGetInput {
    #[serde(alias = "taskId")]
    task_id: String,
}

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &'static str {
        "TaskGet"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TaskGet".to_string(),
            description: "Get full details of a task by ID.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "task_id": { "type": "string", "description": "Task ID to retrieve" }
                }),
                required: vec!["task_id".to_string()],
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

        let params: TaskGetInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let store = task_store().read().await;
        match store.get(&params.task_id) {
            Some(task) => ToolResult::success(
                id,
                serde_json::to_string_pretty(&task.to_full_value()).unwrap_or_default(),
            ),
            None => ToolResult::success(
                id,
                serde_json::to_string_pretty(&serde_json::Value::Null).unwrap_or_default(),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskUpdate
// ---------------------------------------------------------------------------

pub struct TaskUpdateTool;

#[derive(Debug, Deserialize)]
struct TaskUpdateInput {
    #[serde(alias = "taskId")]
    task_id: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default, rename = "addBlocks")]
    add_blocks: Option<Vec<String>>,
    #[serde(default, rename = "addBlockedBy")]
    add_blocked_by: Option<Vec<String>>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    output: Option<String>,
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &'static str {
        "TaskUpdate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TaskUpdate".to_string(),
            description: "Update a task's properties (status, subject, description, etc.)."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "task_id": { "type": "string", "description": "Task ID to update" },
                    "subject": { "type": "string" },
                    "description": { "type": "string" },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "deleted", "failed"]
                    },
                    "owner": { "type": "string" },
                    "addBlocks": { "type": "array", "items": { "type": "string" } },
                    "addBlockedBy": { "type": "array", "items": { "type": "string" } },
                    "metadata": { "type": "object" },
                    "output": { "type": "string" }
                }),
                required: vec!["task_id".to_string()],
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

        let params: TaskUpdateInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let mut store = task_store().write().await;
        let task = match store.get_mut(&params.task_id) {
            Some(t) => t,
            None => return ToolResult::error(id, format!("Task '{}' not found", params.task_id)),
        };

        let mut updated_fields: Vec<&str> = vec![];

        if let Some(subject) = &params.subject {
            task.subject = subject.clone();
            updated_fields.push("subject");
        }
        if let Some(desc) = &params.description {
            task.description = desc.clone();
            updated_fields.push("description");
        }
        if let Some(status_str) = &params.status {
            task.status = match status_str.as_str() {
                "pending" => TaskStatus::Pending,
                "in_progress" | "in-progress" => TaskStatus::InProgress,
                "completed" => TaskStatus::Completed,
                "deleted" => TaskStatus::Deleted,
                "running" => TaskStatus::Running,
                "failed" => TaskStatus::Failed,
                other => return ToolResult::error(id, format!("Unknown status: {}", other)),
            };
            updated_fields.push("status");
        }
        if let Some(owner) = &params.owner {
            task.owner = Some(owner.clone());
            updated_fields.push("owner");
        }
        if let Some(blocks) = &params.add_blocks {
            for b in blocks {
                if !task.blocks.contains(b) {
                    task.blocks.push(b.clone());
                }
            }
            updated_fields.push("blocks");
        }
        if let Some(blocked_by) = &params.add_blocked_by {
            for b in blocked_by {
                if !task.blocked_by.contains(b) {
                    task.blocked_by.push(b.clone());
                }
            }
            updated_fields.push("blocked_by");
        }
        if let Some(meta) = &params.metadata {
            task.metadata = Some(meta.clone());
            updated_fields.push("metadata");
        }
        if let Some(out) = &params.output {
            task.output = Some(out.clone());
            updated_fields.push("output");
        }

        task.updated_at = chrono::Utc::now();

        let task_id = task.id.clone();
        let task_deleted = task.status == TaskStatus::Deleted;
        drop(store);

        if task_deleted {
            task_store().write().await.remove(&task_id);
        }

        ToolResult::success(
            id,
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "task_id": task_id,
                "updated_fields": updated_fields,
            }))
            .unwrap_or_default(),
        )
    }
}

// ---------------------------------------------------------------------------
// TaskList
// ---------------------------------------------------------------------------

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &'static str {
        "TaskList"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TaskList".to_string(),
            description: "List all active tasks (excluding deleted/completed).".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "include_completed": {
                        "type": "boolean",
                        "description": "Include completed tasks (default false)"
                    }
                }),
                required: vec![],
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

        let include_completed = input
            .get("include_completed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let store = task_store().read().await;
        let tasks: Vec<serde_json::Value> = store
            .values()
            .filter(|task| match task.status {
                TaskStatus::Deleted => false,
                TaskStatus::Completed => include_completed,
                _ => true,
            })
            .map(|task| task.to_summary_value())
            .collect();

        ToolResult::success(id, serde_json::to_string_pretty(&tasks).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// TaskStop
// ---------------------------------------------------------------------------

pub struct TaskStopTool;

#[derive(Debug, Deserialize)]
struct TaskStopInput {
    task_id: String,
}

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &'static str {
        "TaskStop"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TaskStop".to_string(),
            description: "Stop a running background task.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "task_id": { "type": "string", "description": "ID of the task to stop" }
                }),
                required: vec!["task_id".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let params: TaskStopInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let mut store = task_store().write().await;
        match store.get_mut(&params.task_id) {
            Some(task) => {
                if task.status != TaskStatus::Running && task.status != TaskStatus::InProgress {
                    return ToolResult::error(
                        id,
                        format!(
                            "Task '{}' is not running (status: {})",
                            params.task_id, task.status
                        ),
                    );
                }
                task.status = TaskStatus::Completed;
                task.updated_at = chrono::Utc::now();
                ToolResult::success(
                    id,
                    serde_json::to_string_pretty(&serde_json::json!({
                        "message": "Task stopped",
                        "task_id": params.task_id,
                    }))
                    .unwrap_or_default(),
                )
            }
            None => ToolResult::error(id, format!("Task '{}' not found", params.task_id)),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskOutput
// ---------------------------------------------------------------------------

pub struct TaskOutputTool;

#[derive(Debug, Deserialize)]
struct TaskOutputInput {
    task_id: String,
    #[serde(default = "default_block")]
    block: bool,
}

fn default_block() -> bool {
    true
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &'static str {
        "TaskOutput"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "TaskOutput".to_string(),
            description: "Get the output of a task.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "task_id": { "type": "string", "description": "Task ID to get output for" },
                    "block": { "type": "boolean", "description": "Wait for task to complete (default true)" }
                }),
                required: vec!["task_id".to_string()],
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

        let params: TaskOutputInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let store = task_store().read().await;
        match store.get(&params.task_id) {
            Some(task) => {
                let retrieval_status = match &task.status {
                    TaskStatus::Completed | TaskStatus::Failed => "success",
                    TaskStatus::Running | TaskStatus::InProgress => {
                        if params.block {
                            "success"
                        } else {
                            "not_ready"
                        }
                    }
                    _ => "success",
                };
                ToolResult::success(
                    id,
                    serde_json::to_string_pretty(&serde_json::json!({
                        "retrieval_status": retrieval_status,
                        "task": task.to_full_value(),
                    }))
                    .unwrap_or_default(),
                )
            }
            None => ToolResult::error(id, format!("Task '{}' not found", params.task_id)),
        }
    }
}
