use async_trait::async_trait;
use piko_types::tool::{ToolCall, ToolResult};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    TextChunk(String),
    ThinkingChunk(String),
    ToolCallStarted(ToolCall),
    ToolCallCompleted {
        call: ToolCall,
        result: ToolResult,
    },
    TurnComplete {
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_tokens: u32,
        cache_read_tokens: u32,
    },
    Error(String),
    /// API returned 429; retry_after is seconds until the limit resets (if provided).
    RateLimit {
        retry_after: Option<u64>,
    },
}

#[async_trait]
pub trait OutputSink: Send + Sync {
    async fn emit(&self, event: AgentEvent);
}

pub struct StdoutSink;

#[async_trait]
impl OutputSink for StdoutSink {
    async fn emit(&self, event: AgentEvent) {
        match event {
            AgentEvent::TextChunk(text) => {
                print!("{}", text);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            AgentEvent::ThinkingChunk(text) => {
                eprint!("{}", text);
                use std::io::Write;
                let _ = std::io::stderr().flush();
            }
            AgentEvent::ToolCallStarted(call) => {
                eprintln!("\n[{}] running...", call.name);
            }
            AgentEvent::ToolCallCompleted { call, result } => {
                if result.is_error {
                    eprintln!("[{}] error: {}", call.name, result.content);
                }
            }
            AgentEvent::TurnComplete {
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
            } => {
                eprintln!(
                    "\n[tokens: in={} out={} cache_write={} cache_read={}]",
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens
                );
            }
            AgentEvent::Error(msg) => {
                eprintln!("\nerror: {}", msg);
            }
            AgentEvent::RateLimit { retry_after } => {
                if let Some(secs) = retry_after {
                    eprintln!("\nrate limited · resets in {}s", secs);
                } else {
                    eprintln!("\nrate limited");
                }
            }
        }
    }
}

pub struct SilentSink;

#[async_trait]
impl OutputSink for SilentSink {
    async fn emit(&self, _event: AgentEvent) {}
}
