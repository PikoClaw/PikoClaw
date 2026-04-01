use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::debug;

pub struct BashTool;

#[derive(Debug, Deserialize)]
struct BashInput {
    command: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    120_000
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "Bash"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Bash".to_string(),
            description: "Executes a bash command and returns stdout, stderr, and exit code. Use for running shell commands, scripts, and system operations.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Timeout in milliseconds (default: 120000)",
                        "default": 120000
                    }
                }),
                required: vec!["command".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let parsed: BashInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        debug!("executing bash command: {}", parsed.command);

        let tool_use_id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let duration = Duration::from_millis(parsed.timeout_ms);

        let fut = async {
            Command::new("bash")
                .arg("-c")
                .arg(&parsed.command)
                .current_dir(&ctx.cwd)
                .output()
                .await
        };

        let result = tokio::select! {
            r = fut => r,
            _ = ctx.cancellation.cancelled() => {
                return ToolResult::error(tool_use_id, "command cancelled");
            }
        };

        match timeout(duration, async { result }).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                if exit_code == 0 {
                    let content = if stderr.is_empty() {
                        stdout
                    } else {
                        format!("{}\nstderr: {}", stdout, stderr)
                    };
                    ToolResult::success(tool_use_id, content.trim_end().to_string())
                } else {
                    ToolResult::error(
                        tool_use_id,
                        format!(
                            "exit code {}\nstdout: {}\nstderr: {}",
                            exit_code,
                            stdout.trim_end(),
                            stderr.trim_end()
                        ),
                    )
                }
            }
            Ok(Err(e)) => ToolResult::error(tool_use_id, format!("failed to execute: {}", e)),
            Err(_) => ToolResult::error(
                tool_use_id,
                format!("command timed out after {}ms", parsed.timeout_ms),
            ),
        }
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        let cmd = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        format!("run bash command: {}", cmd)
    }
}
