use async_trait::async_trait;
use piko_config::config::PermissionMode;
use piko_permissions::checker::{PermissionChecker, PermissionDecision, PermissionRequest};
use piko_permissions::policy::PermissionPolicy;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub struct PermissionAsk {
    pub request: PermissionRequest,
    pub reply: oneshot::Sender<PermissionDecision>,
}

pub struct TuiPermissionChecker {
    policy: Arc<PermissionPolicy>,
    ask_tx: mpsc::UnboundedSender<PermissionAsk>,
}

impl TuiPermissionChecker {
    pub fn new(policy: PermissionPolicy, ask_tx: mpsc::UnboundedSender<PermissionAsk>) -> Self {
        Self {
            policy: Arc::new(policy),
            ask_tx,
        }
    }
}

#[async_trait]
impl PermissionChecker for TuiPermissionChecker {
    async fn check(&self, request: &PermissionRequest) -> PermissionDecision {
        let input_str = serde_json::to_string(&request.input).unwrap_or_default();
        let mode = self.policy.lookup(&request.tool_name, &input_str);

        match mode {
            PermissionMode::Allow => PermissionDecision::Allow,
            PermissionMode::Deny => PermissionDecision::Deny,
            PermissionMode::Ask => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let ask = PermissionAsk {
                    request: request.clone(),
                    reply: reply_tx,
                };
                if self.ask_tx.send(ask).is_err() {
                    return PermissionDecision::Deny;
                }
                reply_rx.await.unwrap_or(PermissionDecision::Deny)
            }
        }
    }
}
