use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot, Mutex};

pub struct PlanModeExitRequest {
    pub reply: oneshot::Sender<bool>,
}

pub type PlanModeExitTx = mpsc::UnboundedSender<PlanModeExitRequest>;

pub struct EnterPlanModeTool {
    plan_mode: Arc<AtomicBool>,
}

impl EnterPlanModeTool {
    pub fn new(plan_mode: Arc<AtomicBool>) -> Self {
        Self { plan_mode }
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &'static str {
        "enter_plan_mode"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "enter_plan_mode".to_string(),
            description: "Enter plan mode. In plan mode, you can read files and analyze code but cannot execute commands or modify files. Use this to think through a solution before making changes.".to_string(),
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
        self.plan_mode.store(true, Ordering::SeqCst);
        ToolResult::success(
            id,
            "Entered plan mode. You can now read and analyze without making changes.",
        )
    }
}

pub struct ExitPlanModeTool {
    plan_mode: Arc<AtomicBool>,
    exit_tx: Arc<Mutex<Option<PlanModeExitTx>>>,
}

impl ExitPlanModeTool {
    pub fn new(plan_mode: Arc<AtomicBool>, exit_tx: PlanModeExitTx) -> Self {
        Self {
            plan_mode,
            exit_tx: Arc::new(Mutex::new(Some(exit_tx))),
        }
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &'static str {
        "exit_plan_mode"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "exit_plan_mode".to_string(),
            description: "Request to exit plan mode to begin making changes. The user must approve this request before you can execute commands or write files.".to_string(),
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

        let tx_guard = self.exit_tx.lock().await;
        let tx = match tx_guard.as_ref() {
            Some(t) => t.clone(),
            None => {
                return ToolResult::error(id, "no plan mode exit channel available");
            }
        };
        drop(tx_guard);

        let (reply_tx, reply_rx) = oneshot::channel();
        if tx.send(PlanModeExitRequest { reply: reply_tx }).is_err() {
            return ToolResult::error(id, "failed to send plan mode exit request");
        }

        match reply_rx.await {
            Ok(true) => {
                self.plan_mode.store(false, Ordering::SeqCst);
                ToolResult::success(id, "Exited plan mode. You can now make changes.")
            }
            Ok(false) | Err(_) => {
                ToolResult::error(id, "User declined to exit plan mode. Continue planning.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_trait::ToolContext;
    use std::path::PathBuf;
    use tokio::sync::mpsc;

    fn make_ctx() -> ToolContext {
        ToolContext::new(PathBuf::from("/tmp"))
    }

    fn make_input(id: &str) -> serde_json::Value {
        serde_json::json!({ "__tool_use_id": id })
    }

    #[tokio::test]
    async fn enter_plan_mode_sets_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        let tool = EnterPlanModeTool::new(Arc::clone(&flag));
        let result = tool.execute(make_input("id1"), &make_ctx()).await;
        assert!(!result.is_error);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn enter_plan_mode_is_idempotent() {
        let flag = Arc::new(AtomicBool::new(true));
        let tool = EnterPlanModeTool::new(Arc::clone(&flag));
        let result = tool.execute(make_input("id2"), &make_ctx()).await;
        assert!(!result.is_error);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn exit_plan_mode_approved_clears_flag() {
        let flag = Arc::new(AtomicBool::new(true));
        let (tx, mut rx) = mpsc::unbounded_channel::<PlanModeExitRequest>();
        let tool = ExitPlanModeTool::new(Arc::clone(&flag), tx);

        let handle = tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                let _ = req.reply.send(true);
            }
        });

        let result = tool.execute(make_input("id3"), &make_ctx()).await;
        handle.await.unwrap();

        assert!(!result.is_error);
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn exit_plan_mode_denied_keeps_flag() {
        let flag = Arc::new(AtomicBool::new(true));
        let (tx, mut rx) = mpsc::unbounded_channel::<PlanModeExitRequest>();
        let tool = ExitPlanModeTool::new(Arc::clone(&flag), tx);

        let handle = tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                let _ = req.reply.send(false);
            }
        });

        let result = tool.execute(make_input("id4"), &make_ctx()).await;
        handle.await.unwrap();

        assert!(result.is_error);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn exit_plan_mode_no_channel_returns_error() {
        let flag = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::unbounded_channel::<PlanModeExitRequest>();
        drop(rx);
        let tool = ExitPlanModeTool::new(Arc::clone(&flag), tx);
        let result = tool.execute(make_input("id5"), &make_ctx()).await;
        assert!(result.is_error);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn enter_tool_name_is_correct() {
        let flag = Arc::new(AtomicBool::new(false));
        let tool = EnterPlanModeTool::new(flag);
        assert_eq!(tool.name(), "enter_plan_mode");
        assert!(tool.is_read_only());
    }

    #[test]
    fn exit_tool_name_is_correct() {
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, _rx) = mpsc::unbounded_channel::<PlanModeExitRequest>();
        let tool = ExitPlanModeTool::new(flag, tx);
        assert_eq!(tool.name(), "exit_plan_mode");
        assert!(tool.is_read_only());
    }

    #[test]
    fn enter_tool_definition_has_description() {
        let flag = Arc::new(AtomicBool::new(false));
        let tool = EnterPlanModeTool::new(flag);
        let def = tool.definition();
        assert_eq!(def.name, "enter_plan_mode");
        assert!(def.description.contains("plan mode"));
    }

    #[test]
    fn exit_tool_definition_has_description() {
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, _rx) = mpsc::unbounded_channel::<PlanModeExitRequest>();
        let tool = ExitPlanModeTool::new(flag, tx);
        let def = tool.definition();
        assert_eq!(def.name, "exit_plan_mode");
        assert!(def.description.contains("plan mode"));
    }
}
