use piko_types::{message::Message, model::ModelId, tool::ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub budget_tokens: u32,
}

impl ThinkingConfig {
    pub fn enabled(budget_tokens: u32) -> Self {
        Self {
            kind: "enabled".to_string(),
            budget_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub control_type: String,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            control_type: "ephemeral".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesRequest {
    pub model: ModelId,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemBlock>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip)]
    pub betas: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
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
            thinking: None,
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(vec![SystemBlock {
            block_type: "text".to_string(),
            text: system.into(),
            cache_control: Some(CacheControl::ephemeral()),
        }]);
        self
    }

    pub fn with_system_no_cache(mut self, system: impl Into<String>) -> Self {
        self.system = Some(vec![SystemBlock {
            block_type: "text".to_string(),
            text: system.into(),
            cache_control: None,
        }]);
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        let mut values: Vec<serde_json::Value> = tools
            .into_iter()
            .filter_map(|t| serde_json::to_value(t).ok())
            .collect();

        if let Some(last) = values.last_mut() {
            if let Some(obj) = last.as_object_mut() {
                obj.insert(
                    "cache_control".to_string(),
                    serde_json::json!({ "type": "ephemeral" }),
                );
            }
        }
        self.tools = values;
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

    pub fn with_thinking(mut self, budget_tokens: u32) -> Self {
        self.thinking = Some(ThinkingConfig::enabled(budget_tokens));
        let beta = "interleaved-thinking-2025-05-14".to_string();
        match &mut self.betas {
            Some(betas) => {
                if !betas.contains(&beta) {
                    betas.push(beta);
                }
            }
            None => self.betas = Some(vec![beta]),
        }
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn messages_with_cache(&self) -> Vec<Value> {
        let mut values: Vec<Value> = self
            .messages
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect();

        if let Some(last_msg) = values.last_mut() {
            if let Some(content) = last_msg.get_mut("content") {
                if let Some(arr) = content.as_array_mut() {
                    if let Some(last_block) = arr.last_mut() {
                        if let Some(obj) = last_block.as_object_mut() {
                            obj.insert(
                                "cache_control".to_string(),
                                serde_json::json!({ "type": "ephemeral" }),
                            );
                        }
                    }
                }
            }
        }

        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piko_types::model::ModelId;

    #[test]
    fn test_thinking_config_serializes_correctly() {
        let cfg = ThinkingConfig::enabled(10000);
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["type"], "enabled");
        assert_eq!(json["budget_tokens"], 10000);
    }

    #[test]
    fn test_with_thinking_sets_field_and_beta() {
        let req = MessagesRequest::new(ModelId::default(), vec![]).with_thinking(5000);
        assert!(req.thinking.is_some());
        assert_eq!(req.thinking.as_ref().unwrap().budget_tokens, 5000);
        let betas = req.betas.as_ref().unwrap();
        assert!(betas.contains(&"interleaved-thinking-2025-05-14".to_string()));
    }

    #[test]
    fn test_with_thinking_appends_to_existing_betas() {
        let req = MessagesRequest::new(ModelId::default(), vec![])
            .with_betas(vec!["web-search-2025-03-05".to_string()])
            .with_thinking(5000);
        let betas = req.betas.as_ref().unwrap();
        assert!(betas.contains(&"web-search-2025-03-05".to_string()));
        assert!(betas.contains(&"interleaved-thinking-2025-05-14".to_string()));
    }

    #[test]
    fn test_with_thinking_does_not_duplicate_beta() {
        let req = MessagesRequest::new(ModelId::default(), vec![])
            .with_thinking(5000)
            .with_thinking(5000);
        let betas = req.betas.as_ref().unwrap();
        let count = betas
            .iter()
            .filter(|b| *b == "interleaved-thinking-2025-05-14")
            .count();
        assert_eq!(count, 1);
    }
}
