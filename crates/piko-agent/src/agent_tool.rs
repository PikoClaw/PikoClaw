use crate::agent::AgentConfig;
use crate::agent_loop::run_turn;
use crate::context::ConversationContext;
use crate::output::SilentSink;
use async_trait::async_trait;
use piko_api::AnthropicClient;
use piko_tools::registry::ToolRegistry;
use piko_tools::tool_trait::{Tool, ToolContext};
use piko_types::tool::{ToolDefinition, ToolResult};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct AgentTool {
    client: Arc<AnthropicClient>,
    config: Arc<AgentConfig>,
}

impl AgentTool {
    pub fn new(client: Arc<AnthropicClient>, config: Arc<AgentConfig>) -> Self {
        Self { client, config }
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &'static str {
        "Agent"
    }

    fn definition(&self) -> ToolDefinition {
        use piko_types::tool::ToolInputSchema;
        ToolDefinition {
            name: "Agent".to_string(),
            description: "Launch a sub-agent to handle a complex multi-step task. The sub-agent has access to all tools and runs in an isolated conversation context. Returns the final text response from the sub-agent.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "prompt": {
                        "type": "string",
                        "description": "The task or question for the sub-agent to handle."
                    },
                    "system_prompt": {
                        "type": "string",
                        "description": "Optional system prompt to guide the sub-agent's behavior."
                    }
                }),
                required: vec!["prompt".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let prompt = match input.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::error(id, "missing required field: prompt".to_string()),
        };

        let system_prompt = input
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let sub_config = AgentConfig {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            max_turns: Some(30),
            cwd: ctx.cwd.clone(),
            system_prompt: system_prompt.clone(),
            bypass_permissions: self.config.bypass_permissions,
            extended_thinking: self.config.extended_thinking,
            thinking_budget_tokens: self.config.thinking_budget_tokens,
        };

        let tools = Arc::new(ToolRegistry::with_defaults());
        let mut context = ConversationContext::new();
        if let Some(sp) = system_prompt {
            context.system_prompt = Some(sp);
        }
        context.push_user(&prompt);

        let sink: Arc<dyn crate::output::OutputSink> = Arc::new(SilentSink);
        let cancellation = CancellationToken::new();

        match run_turn(
            &self.client,
            &tools,
            &piko_permissions::default::DefaultPermissionChecker::bypass(),
            &mut context,
            &sub_config,
            sink,
            cancellation,
        )
        .await
        {
            Ok(text) => ToolResult::success(id, text),
            Err(e) => ToolResult::error(id, format!("sub-agent error: {}", e)),
        }
    }

    fn description_for_permission(&self, input: &serde_json::Value) -> String {
        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown task)");
        format!("run sub-agent: {}", &prompt[..prompt.len().min(80)])
    }
}
