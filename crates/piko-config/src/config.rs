use piko_types::model::ModelId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PikoConfig {
    pub api: ApiConfig,
    pub permissions: PermissionsConfig,
    pub tui: TuiConfig,
    pub mcp: McpConfig,
}

fn default_thinking_budget_tokens() -> u32 {
    10000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub model: ModelId,
    pub max_tokens: u32,
    pub base_url: String,
    pub api_key: Option<String>,
    /// Bearer token for third-party providers (e.g. OpenRouter).
    /// Populated from ANTHROPIC_AUTH_TOKEN. When set, `x-api-key` is replaced
    /// with `Authorization: Bearer <token>` — no ANTHROPIC_API_KEY required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    /// Maximum session cost in USD. When accumulated cost reaches this limit, the session stops.
    /// If None, no budget limit is enforced.
    #[serde(default)]
    pub max_budget_usd: Option<f64>,
    #[serde(default)]
    pub extended_thinking: bool,
    #[serde(default = "default_thinking_budget_tokens")]
    pub thinking_budget_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsConfig {
    pub default_mode: PermissionMode,
    pub bash: PermissionMode,
    pub file_write: PermissionMode,
    pub file_read: PermissionMode,
    pub web_fetch: PermissionMode,
    pub rules: Vec<PermissionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub tool: String,
    pub pattern: String,
    pub decision: PermissionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    pub theme: String,
    pub syntax_highlight: bool,
    /// Set to true after the first-run onboarding wizard completes.
    #[serde(default)]
    pub has_completed_onboarding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
    },
    Sse {
        url: String,
    },
}
