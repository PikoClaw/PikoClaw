use crate::config::{ApiConfig, PermissionMode, PermissionsConfig, TuiConfig};
use piko_types::model::ModelId;

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            model: ModelId::default(),
            max_tokens: 8192,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: None,
            auth_token: None,
            max_budget_usd: None,
            extended_thinking: false,
            thinking_budget_tokens: 10000,
        }
    }
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            default_mode: PermissionMode::Ask,
            bash: PermissionMode::Ask,
            file_write: PermissionMode::Ask,
            file_read: PermissionMode::Allow,
            web_fetch: PermissionMode::Ask,
            rules: Vec::new(),
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            syntax_highlight: true,
            has_completed_onboarding: false,
        }
    }
}
