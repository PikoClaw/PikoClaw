use crate::error::ApiError;
use crate::response::{null_as_zero, StopReason, Usage};
use futures_util::Stream;
use piko_types::message::ContentBlock;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ApiError>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: Delta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: Option<DeltaUsage>,
    },
    MessageStop,
    Ping,
    Error {
        error: StreamError,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartData {
    pub id: String,
    pub model: String,
    pub role: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delta {
    TextDelta {
        text: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaUsage {
    #[serde(default, deserialize_with = "null_as_zero")]
    pub input_tokens: u32,
    #[serde(default, deserialize_with = "null_as_zero")]
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

pub fn parse_sse_line(event_type: &str, data: &str) -> Result<Option<StreamEvent>, ApiError> {
    if data == "[DONE]" {
        return Ok(None);
    }
    if event_type == "ping" {
        return Ok(Some(StreamEvent::Ping));
    }
    let event: StreamEvent = serde_json::from_str(data).map_err(|e| {
        // Truncate the raw data in the error so the TUI status bar doesn't overflow
        let preview = if data.len() > 120 {
            format!("{}…", &data[..120])
        } else {
            data.to_string()
        };
        ApiError::Sse(format!("failed to parse event '{}': {}", preview, e))
    })?;
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_delta_deserializes() {
        let json = r#"{"type":"thinking_delta","thinking":"Let me think..."}"#;
        let delta: Delta = serde_json::from_str(json).unwrap();
        assert!(
            matches!(delta, Delta::ThinkingDelta { thinking } if thinking == "Let me think...")
        );
    }

    #[test]
    fn test_content_block_start_with_thinking_parses() {
        use piko_types::message::ContentBlock;
        let json = r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#;
        let event: StreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::Thinking { .. }
            }
        ));
    }

    #[test]
    fn test_text_delta_still_works() {
        let json = r#"{"type":"text_delta","text":"hello"}"#;
        let delta: Delta = serde_json::from_str(json).unwrap();
        assert!(matches!(delta, Delta::TextDelta { text } if text == "hello"));
    }
}
