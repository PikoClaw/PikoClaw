use piko_types::{message::Message, model::ModelId, tool::ToolDefinition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesRequest {
    pub model: ModelId,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub betas: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

impl MessagesRequest {
    pub fn new(model: ModelId, messages: Vec<Message>) -> Self {
        Self {
            model,
            max_tokens: 8192,
            messages,
            system: None,
            tools: Vec::new(),
            tool_choice: None,
            stream: true,
            temperature: None,
            betas: None,
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools
            .into_iter()
            .filter_map(|t| serde_json::to_value(t).ok())
            .collect();
        self
    }

    pub fn with_raw_tool(mut self, tool: serde_json::Value) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn with_betas(mut self, betas: Vec<String>) -> Self {
        self.betas = Some(betas);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}
