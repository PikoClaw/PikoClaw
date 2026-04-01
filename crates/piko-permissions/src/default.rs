use crate::checker::{PermissionChecker, PermissionDecision, PermissionRequest};
use crate::policy::PermissionPolicy;
use async_trait::async_trait;
use piko_config::config::PermissionMode;
use std::io::{self, Write};
use std::sync::Arc;

pub struct DefaultPermissionChecker {
    policy: Arc<PermissionPolicy>,
    bypass_all: bool,
}

impl DefaultPermissionChecker {
    pub fn new(policy: PermissionPolicy) -> Self {
        Self {
            policy: Arc::new(policy),
            bypass_all: false,
        }
    }

    pub fn bypass() -> Self {
        let policy =
            PermissionPolicy::from_config(&piko_config::config::PermissionsConfig::default());
        Self {
            policy: Arc::new(policy),
            bypass_all: true,
        }
    }

    fn prompt_user(request: &PermissionRequest) -> PermissionDecision {
        let input_display = serde_json::to_string_pretty(&request.input)
            .unwrap_or_else(|_| request.input.to_string());

        eprintln!("\n[Permission Required]");
        eprintln!("Tool: {}", request.tool_name);
        eprintln!("Action: {}", request.description);
        eprintln!("Input: {}", input_display);
        eprint!("Allow? [y]es / [n]o / [a]lways / [d]eny always: ");

        let _ = io::stderr().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return PermissionDecision::Deny;
        }

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => PermissionDecision::Allow,
            "a" | "always" => PermissionDecision::AllowAlways,
            "d" | "deny always" => PermissionDecision::DenyAlways,
            _ => PermissionDecision::Deny,
        }
    }
}

#[async_trait]
impl PermissionChecker for DefaultPermissionChecker {
    async fn check(&self, request: &PermissionRequest) -> PermissionDecision {
        if self.bypass_all {
            return PermissionDecision::Allow;
        }

        let input_str = serde_json::to_string(&request.input).unwrap_or_default();
        let mode = self.policy.lookup(&request.tool_name, &input_str);

        match mode {
            PermissionMode::Allow => PermissionDecision::Allow,
            PermissionMode::Deny => PermissionDecision::Deny,
            PermissionMode::Ask => Self::prompt_user(request),
        }
    }
}
