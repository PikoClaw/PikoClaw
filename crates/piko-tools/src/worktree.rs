// Worktree tools: create and exit git worktrees for isolated work sessions.
//
// EnterWorktreeTool – create a new git worktree with an optional branch name,
//                     switching the session's working directory to it.
// ExitWorktreeTool  – exit the current worktree, optionally removing it, and
//                     restore the original working directory.

use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::debug;

// ---------------------------------------------------------------------------
// Session-level state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WorktreeSession {
    pub original_cwd: PathBuf,
    pub worktree_path: PathBuf,
    pub branch: Option<String>,
    pub original_head: Option<String>,
}

static WORKTREE_SESSION: OnceLock<Arc<RwLock<Option<WorktreeSession>>>> = OnceLock::new();

fn worktree_session() -> &'static Arc<RwLock<Option<WorktreeSession>>> {
    WORKTREE_SESSION.get_or_init(|| Arc::new(RwLock::new(None)))
}

// ---------------------------------------------------------------------------
// EnterWorktreeTool
// ---------------------------------------------------------------------------

pub struct EnterWorktreeTool;

#[derive(Debug, Deserialize)]
struct EnterWorktreeInput {
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    post_create_command: Option<String>,
}

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &'static str {
        "EnterWorktree"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "EnterWorktree".to_string(),
            description: "Create a new git worktree and switch the session's working directory \
                to it. This gives you an isolated environment to experiment or work on a feature \
                without affecting the main working tree. \
                Use ExitWorktree to return to the original directory."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "branch": {
                        "type": "string",
                        "description": "Branch name to create. Defaults to a timestamped name."
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional path for the worktree directory. Defaults to .worktrees/<branch>."
                    },
                    "post_create_command": {
                        "type": "string",
                        "description": "Optional command to run inside the new worktree after creation (e.g. 'npm install')."
                    }
                }),
                required: vec![],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let params: EnterWorktreeInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        {
            let session = worktree_session().read().await;
            if session.is_some() {
                return ToolResult::error(
                    id,
                    "Already in a worktree session. Call ExitWorktree first.",
                );
            }
        }

        let branch = params.branch.clone().unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let s = secs % 60;
            let m = (secs / 60) % 60;
            let h = (secs / 3600) % 24;
            let days = secs / 86400;
            let year = 1970 + days / 365;
            let day_of_year = days % 365;
            let month = day_of_year / 30 + 1;
            let day = day_of_year % 30 + 1;
            format!(
                "pikoclaw-{:04}{:02}{:02}-{:02}{:02}{:02}",
                year, month, day, h, m, s
            )
        });

        let worktree_path = if let Some(p) = params.path {
            ctx.cwd.join(p)
        } else {
            ctx.cwd.join(".worktrees").join(&branch)
        };

        let head_result = run_git(&ctx.cwd, &["rev-parse", "HEAD"]).await;
        let original_head = match &head_result {
            Ok(h) => Some(h.trim().to_string()),
            Err(e) => {
                let msg = e.to_lowercase();
                if msg.contains("not a git repository") || msg.contains("fatal") {
                    return ToolResult::error(
                        id,
                        format!(
                            "Cannot create worktree: '{}' is not inside a git repository.",
                            ctx.cwd.display()
                        ),
                    );
                }
                None
            }
        };

        if worktree_path.exists() {
            return ToolResult::error(
                id,
                format!(
                    "Cannot create worktree: the path '{}' already exists.",
                    worktree_path.display()
                ),
            );
        }

        let worktree_str = worktree_path.to_string_lossy().to_string();
        let result = run_git(&ctx.cwd, &["worktree", "add", "-b", &branch, &worktree_str]).await;

        match result {
            Err(e) => {
                let msg = e.trim().to_string();
                let friendly = if msg.to_lowercase().contains("already exists") {
                    format!(
                        "Failed to create worktree: branch '{}' already exists.",
                        branch
                    )
                } else if msg.to_lowercase().contains("not a git repository") {
                    format!(
                        "Failed to create worktree: '{}' is not inside a git repository.",
                        ctx.cwd.display()
                    )
                } else {
                    format!("Failed to create worktree: {}", msg)
                };
                ToolResult::error(id, friendly)
            }
            Ok(_) => {
                debug!(branch = %branch, path = %worktree_path.display(), "Created worktree");

                *worktree_session().write().await = Some(WorktreeSession {
                    original_cwd: ctx.cwd.clone(),
                    worktree_path: worktree_path.clone(),
                    branch: Some(branch.clone()),
                    original_head,
                });

                let post_create_output = if let Some(cmd) = params.post_create_command {
                    let shell_result = if cfg!(target_os = "windows") {
                        tokio::process::Command::new("cmd")
                            .args(["/C", &cmd])
                            .current_dir(&worktree_path)
                            .output()
                            .await
                    } else {
                        tokio::process::Command::new("sh")
                            .args(["-c", &cmd])
                            .current_dir(&worktree_path)
                            .output()
                            .await
                    };
                    match shell_result {
                        Ok(out) if out.status.success() => {
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            format!(
                                "\nPost-create command '{}' completed.{}",
                                cmd,
                                if stdout.trim().is_empty() {
                                    String::new()
                                } else {
                                    format!("\nOutput: {}", stdout.trim())
                                }
                            )
                        }
                        Ok(out) => {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            format!(
                                "\nPost-create command '{}' exited with error.\nStderr: {}",
                                cmd,
                                stderr.trim()
                            )
                        }
                        Err(e) => format!("\nCould not run post-create command '{}': {}", cmd, e),
                    }
                } else {
                    String::new()
                };

                ToolResult::success(
                    id,
                    format!(
                        "Created worktree at {} on branch '{}'.\n\
                         The working directory is now {}.\n\
                         Use ExitWorktree to return to {}.{}",
                        worktree_path.display(),
                        branch,
                        worktree_path.display(),
                        ctx.cwd.display(),
                        post_create_output,
                    ),
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ExitWorktreeTool
// ---------------------------------------------------------------------------

pub struct ExitWorktreeTool;

#[derive(Debug, Deserialize)]
struct ExitWorktreeInput {
    #[serde(default = "default_action")]
    action: String,
    #[serde(default)]
    discard_changes: bool,
}

fn default_action() -> String {
    "keep".to_string()
}

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &'static str {
        "ExitWorktree"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ExitWorktree".to_string(),
            description: "Exit the current worktree session created by EnterWorktree and restore \
                the original working directory. Use action='keep' to preserve the worktree on \
                disk, or action='remove' to delete it."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "action": {
                        "type": "string",
                        "enum": ["keep", "remove"],
                        "description": "\"keep\" leaves the worktree on disk; \"remove\" deletes it and its branch."
                    },
                    "discard_changes": {
                        "type": "boolean",
                        "description": "Set true when action=remove and the worktree has uncommitted work to discard."
                    }
                }),
                required: vec!["action".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let params: ExitWorktreeInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        let session_guard = worktree_session().read().await;
        let session = match &*session_guard {
            Some(s) => s.clone(),
            None => {
                return ToolResult::error(id, "No active EnterWorktree session to exit.");
            }
        };
        drop(session_guard);

        let worktree_str = session.worktree_path.to_string_lossy().to_string();

        if params.action == "remove" && !params.discard_changes {
            let status = run_git(&session.worktree_path, &["status", "--porcelain"]).await;
            let changed_files = status
                .as_deref()
                .unwrap_or("")
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count();

            let commit_count = if let Some(ref head) = session.original_head {
                let rev = run_git(
                    &session.worktree_path,
                    &["rev-list", "--count", &format!("{}..HEAD", head)],
                )
                .await
                .unwrap_or_default();
                rev.trim().parse::<usize>().unwrap_or(0)
            } else {
                0
            };

            if changed_files > 0 || commit_count > 0 {
                let mut parts = Vec::new();
                if changed_files > 0 {
                    parts.push(format!("{} uncommitted file(s)", changed_files));
                }
                if commit_count > 0 {
                    parts.push(format!("{} commit(s) on the worktree branch", commit_count));
                }
                return ToolResult::error(
                    id,
                    format!(
                        "Worktree has {}. Re-invoke with discard_changes=true \
                         or use action=\"keep\" to preserve the worktree.",
                        parts.join(" and ")
                    ),
                );
            }
        }

        *worktree_session().write().await = None;

        match params.action.as_str() {
            "keep" => {
                let _ = run_git(
                    &session.original_cwd,
                    &[
                        "worktree",
                        "lock",
                        "--reason",
                        "kept by ExitWorktree",
                        &worktree_str,
                    ],
                )
                .await;

                ToolResult::success(
                    id,
                    format!(
                        "Exited worktree. Work preserved at {} on branch {}. \
                         Session is now back in {}.",
                        session.worktree_path.display(),
                        session.branch.as_deref().unwrap_or("(unknown)"),
                        session.original_cwd.display(),
                    ),
                )
            }
            "remove" => {
                let _ = run_git(
                    &session.original_cwd,
                    &["worktree", "remove", "--force", &worktree_str],
                )
                .await;

                if let Some(ref branch) = session.branch {
                    let _ = run_git(&session.original_cwd, &["branch", "-D", branch]).await;
                }

                ToolResult::success(
                    id,
                    format!(
                        "Exited and removed worktree at {}. Session is now back in {}.",
                        session.worktree_path.display(),
                        session.original_cwd.display(),
                    ),
                )
            }
            other => ToolResult::error(
                id,
                format!("Unknown action '{}'. Use 'keep' or 'remove'.", other),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

async fn run_git(cwd: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
