use async_trait::async_trait;
use piko_permissions::checker::{PermissionChecker, PermissionDecision, PermissionRequest};
use piko_permissions::policy::PermissionPolicy;
use piko_config::config::PermissionMode;
use std::sync::Arc;

pub struct TuiPermissionChecker {
    policy: Arc<PermissionPolicy>,
}

impl TuiPermissionChecker {
    pub fn new(policy: PermissionPolicy) -> Self {
        Self {
            policy: Arc::new(policy),
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
            PermissionMode::Ask => PermissionDecision::Allow,
        }
    }
}
