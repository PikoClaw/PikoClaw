use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Allow,
    AllowAlways,
    Deny,
    DenyAlways,
}

#[derive(Debug, Clone)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub description: String,
    pub input: serde_json::Value,
}

#[async_trait]
pub trait PermissionChecker: Send + Sync {
    async fn check(&self, request: &PermissionRequest) -> PermissionDecision;
}
