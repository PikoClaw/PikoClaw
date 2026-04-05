use piko_types::message::ContentBlock;
use serde::{Deserialize, Serialize};

/// Deserialize a JSON value that may be an integer **or** `null` into `u32`.
/// `#[serde(default)]` alone only handles *missing* fields; third-party providers
/// (e.g. OpenRouter) explicitly send `null` for unsupported token-count fields,
/// which would otherwise fail with "expected u32".
pub(crate) fn null_as_zero<'de, D>(d: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u32>::deserialize(d)?.unwrap_or(0))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    pub model: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<StopReason>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    #[serde(default, deserialize_with = "null_as_zero")]
    pub input_tokens: u32,
    #[serde(default, deserialize_with = "null_as_zero")]
    pub output_tokens: u32,
    #[serde(default, deserialize_with = "null_as_zero")]
    pub cache_creation_input_tokens: u32,
    #[serde(default, deserialize_with = "null_as_zero")]
    pub cache_read_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error: ApiErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}
