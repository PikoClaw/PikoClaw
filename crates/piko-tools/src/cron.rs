// Cron tools: schedule recurring and one-shot prompts.
//
// CronCreateTool  – create a new scheduled task (cron expression)
// CronDeleteTool  – remove an existing scheduled task
// CronListTool    – list all scheduled tasks
//
// Cron expression format: "M H DoM Mon DoW" (standard 5-field cron in local
// time). For example:
//   "*/5 * * * *"   = every 5 minutes
//   "30 14 * * 1"   = every Monday at 14:30

use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Local, Timelike};
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// In-memory store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronTask {
    pub id: String,
    pub cron: String,
    pub prompt: String,
    pub recurring: bool,
    pub durable: bool,
    pub created_at: u64,
}

const MAX_TASK_AGE_SECS: u64 = 7 * 24 * 3600;

static CRON_STORE: OnceLock<Arc<RwLock<HashMap<String, CronTask>>>> = OnceLock::new();
static STORE_INITIALIZED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

fn cron_store() -> &'static Arc<RwLock<HashMap<String, CronTask>>> {
    CRON_STORE.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

// ---------------------------------------------------------------------------
// Disk path helpers
// ---------------------------------------------------------------------------

fn scheduled_tasks_path() -> Option<PathBuf> {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".pikoclaw").join("scheduled_tasks.json"))
}

async fn ensure_store_loaded() {
    STORE_INITIALIZED
        .get_or_init(|| async {
            let path = match scheduled_tasks_path() {
                Some(p) => p,
                None => return,
            };

            let data = match tokio::fs::read_to_string(&path).await {
                Ok(d) => d,
                Err(_) => return,
            };

            let tasks: Vec<CronTask> = match serde_json::from_str(&data) {
                Ok(t) => t,
                Err(e) => {
                    debug!("Failed to parse scheduled_tasks.json: {}", e);
                    return;
                }
            };

            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let mut store = cron_store().write().await;
            for task in tasks {
                if now_secs.saturating_sub(task.created_at) > MAX_TASK_AGE_SECS {
                    debug!("Cron task {} expired, skipping on load", task.id);
                    continue;
                }
                store.insert(task.id.clone(), task);
            }
        })
        .await;
}

// ---------------------------------------------------------------------------
// Public scheduler API
// ---------------------------------------------------------------------------

/// Check if a cron expression fires at the given minute-resolution datetime.
pub fn cron_matches(expr: &str, dt: &DateTime<Local>) -> bool {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return false;
    }
    let minute = dt.minute();
    let hour = dt.hour();
    let day = dt.day();
    let month = dt.month();
    let dow = dt.weekday().num_days_from_sunday();

    cron_field_matches(fields[0], minute)
        && cron_field_matches(fields[1], hour)
        && cron_field_matches(fields[2], day)
        && cron_field_matches(fields[3], month)
        && cron_field_matches(fields[4], dow)
}

fn cron_field_matches(field: &str, value: u32) -> bool {
    if field == "*" {
        return true;
    }
    if let Some(step_str) = field.strip_prefix("*/") {
        if let Ok(step) = step_str.parse::<u32>() {
            return step > 0 && value % step == 0;
        }
    }
    for part in field.split(',') {
        if cron_range_matches(part, value) {
            return true;
        }
    }
    false
}

fn cron_range_matches(part: &str, value: u32) -> bool {
    if let Some(dash) = part.find('-') {
        let lo: u32 = part[..dash].parse().unwrap_or(u32::MAX);
        let hi: u32 = part[dash + 1..].parse().unwrap_or(0);
        value >= lo && value <= hi
    } else {
        part.parse::<u32>()
            .is_ok_and(|n| n == value || (n == 7 && value == 0))
    }
}

/// Return all tasks whose cron expression fires at `dt`.
/// One-shot tasks (recurring=false) are removed from the store after being returned.
pub async fn pop_due_tasks(dt: &DateTime<Local>) -> Vec<CronTask> {
    ensure_store_loaded().await;
    let mut store = cron_store().write().await;
    let due: Vec<CronTask> = store
        .values()
        .filter(|t| cron_matches(&t.cron, dt))
        .cloned()
        .collect();
    for t in &due {
        if !t.recurring {
            store.remove(&t.id);
        }
    }
    due
}

// ---------------------------------------------------------------------------
// Cron expression helpers
// ---------------------------------------------------------------------------

fn validate_cron(expr: &str) -> bool {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return false;
    }
    let ranges = [(0u32, 59), (0, 23), (1, 31), (1, 12), (0, 7)];
    for (i, field) in fields.iter().enumerate() {
        if *field == "*" {
            continue;
        }
        if let Some(step) = field.strip_prefix("*/") {
            if step.parse::<u32>().is_err() {
                return false;
            }
            continue;
        }
        let parts: Vec<&str> = field.split('-').collect();
        for part in &parts {
            match part.parse::<u32>() {
                Ok(n) => {
                    if n < ranges[i].0 || n > ranges[i].1 {
                        return false;
                    }
                }
                Err(_) => return false,
            }
        }
    }
    true
}

fn cron_to_human(expr: &str) -> String {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return expr.to_string();
    }

    let (minute, hour, dom, month, dow) = (fields[0], fields[1], fields[2], fields[3], fields[4]);

    if expr == "* * * * *" {
        return "every minute".to_string();
    }
    if let Some(n) = minute.strip_prefix("*/") {
        return format!("every {} minutes", n);
    }
    if hour == "*" && dom == "*" && month == "*" && dow == "*" {
        return format!("at minute {} of every hour", minute);
    }
    if dom == "*" && month == "*" && dow == "*" {
        return format!("daily at {:0>2}:{:0>2}", hour, minute);
    }
    format!("cron({})", expr)
}

async fn persist_tasks_to_disk(store: &HashMap<String, CronTask>) -> Result<(), String> {
    let durable: Vec<&CronTask> = store.values().filter(|t| t.durable).collect();
    let json = serde_json::to_string_pretty(&durable).map_err(|e| e.to_string())?;

    let path =
        scheduled_tasks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;
    let dir = path.parent().ok_or("No parent directory")?;

    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| e.to_string())?;

    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ---------------------------------------------------------------------------
// CronCreate
// ---------------------------------------------------------------------------

pub struct CronCreateTool;

#[derive(Debug, Deserialize)]
struct CronCreateInput {
    cron: String,
    prompt: String,
    #[serde(default = "default_true")]
    recurring: bool,
    #[serde(default)]
    durable: bool,
}

fn default_true() -> bool {
    true
}

#[async_trait]
impl Tool for CronCreateTool {
    fn name(&self) -> &'static str {
        "CronCreate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "CronCreate".to_string(),
            description: "Schedule a recurring or one-shot prompt using a standard 5-field cron \
                expression in local time: \"M H DoM Mon DoW\". Examples:\n\
                - \"*/5 * * * *\" = every 5 minutes\n\
                - \"30 14 * * 1\" = every Monday at 14:30\n\
                Use recurring=false for one-shot (fires once then auto-deletes). \
                Use durable=true to persist across sessions."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "cron": {
                        "type": "string",
                        "description": "5-field cron expression: M H DoM Mon DoW"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The prompt to run at each scheduled time"
                    },
                    "recurring": {
                        "type": "boolean",
                        "description": "true (default) = repeat; false = fire once then delete"
                    },
                    "durable": {
                        "type": "boolean",
                        "description": "true = persist to ~/.pikoclaw/scheduled_tasks.json; false (default) = session only"
                    }
                }),
                required: vec!["cron".to_string(), "prompt".to_string()],
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

        let params: CronCreateInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        if !validate_cron(&params.cron) {
            return ToolResult::error(
                id,
                format!(
                    "Invalid cron expression '{}'. Expected 5 fields: M H DoM Mon DoW.",
                    params.cron
                ),
            );
        }

        ensure_store_loaded().await;

        let mut store = cron_store().write().await;
        if store.len() >= 50 {
            return ToolResult::error(id, "Too many scheduled jobs (max 50). Cancel one first.");
        }

        let task_id = Uuid::new_v4().to_string()[..8].to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let task = CronTask {
            id: task_id.clone(),
            cron: params.cron.clone(),
            prompt: params.prompt.clone(),
            recurring: params.recurring,
            durable: params.durable,
            created_at: now,
        };

        store.insert(task_id.clone(), task);

        if params.durable {
            if let Err(e) = persist_tasks_to_disk(&store).await {
                debug!("Failed to persist cron task to disk: {}", e);
            }
        }

        let human = cron_to_human(&params.cron);
        let where_note = if params.durable {
            "Persisted to ~/.pikoclaw/scheduled_tasks.json"
        } else {
            "Session-only"
        };

        let msg = if params.recurring {
            format!(
                "Scheduled recurring job {} ({}). {}",
                task_id, human, where_note
            )
        } else {
            format!(
                "Scheduled one-shot task {} ({}). {}. Will fire once then auto-delete.",
                task_id, human, where_note
            )
        };

        ToolResult::success(id, msg)
    }
}

// ---------------------------------------------------------------------------
// CronDelete
// ---------------------------------------------------------------------------

pub struct CronDeleteTool;

#[derive(Debug, Deserialize)]
struct CronDeleteInput {
    id: String,
}

#[async_trait]
impl Tool for CronDeleteTool {
    fn name(&self) -> &'static str {
        "CronDelete"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "CronDelete".to_string(),
            description: "Cancel a scheduled cron task by its ID. Use CronList to find the ID."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "id": {
                        "type": "string",
                        "description": "The cron task ID to delete"
                    }
                }),
                required: vec!["id".to_string()],
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

        let params: CronDeleteInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        ensure_store_loaded().await;

        let mut store = cron_store().write().await;
        if let Some(removed) = store.remove(&params.id) {
            if removed.durable {
                if let Err(e) = persist_tasks_to_disk(&store).await {
                    debug!("Failed to update scheduled_tasks.json after delete: {}", e);
                }
            }
            ToolResult::success(id, format!("Deleted cron task '{}'.", params.id))
        } else {
            ToolResult::error(id, format!("Cron task '{}' not found.", params.id))
        }
    }
}

// ---------------------------------------------------------------------------
// CronList
// ---------------------------------------------------------------------------

pub struct CronListTool;

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &'static str {
        "CronList"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "CronList".to_string(),
            description: "List all currently scheduled cron tasks.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({}),
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

        ensure_store_loaded().await;

        let store = cron_store().read().await;

        if store.is_empty() {
            return ToolResult::success(id, "No scheduled cron tasks.");
        }

        let mut tasks: Vec<&CronTask> = store.values().collect();
        tasks.sort_by_key(|t| t.created_at);

        let lines: Vec<String> = tasks
            .iter()
            .map(|t| {
                format!(
                    "{} | {} | {} | recurring={} | durable={} | prompt: {}",
                    t.id,
                    t.cron,
                    cron_to_human(&t.cron),
                    t.recurring,
                    t.durable,
                    if t.prompt.len() > 60 {
                        format!("{}…", &t.prompt[..60])
                    } else {
                        t.prompt.clone()
                    }
                )
            })
            .collect();

        ToolResult::success(
            id,
            format!("Scheduled tasks ({}):\n\n{}", tasks.len(), lines.join("\n")),
        )
    }
}
