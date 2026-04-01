use async_trait::async_trait;
use piko_types::tool::{ToolCall, ToolResult};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    TextChunk(String),
    ToolCallStarted(ToolCall),
    ToolCallCompleted { call: ToolCall, result: ToolResult },
    TurnComplete { input_tokens: u32, output_tokens: u32 },
    Error(String),
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
            AgentEvent::ToolCallStarted(call) => {
                eprintln!("\n[{}] running...", call.name);
            }
            AgentEvent::ToolCallCompleted { call, result } => {
                if result.is_error {
                    eprintln!("[{}] error: {}", call.name, result.content);
                }
            }
            AgentEvent::TurnComplete { input_tokens, output_tokens } => {
                eprintln!("\n[tokens: in={} out={}]", input_tokens, output_tokens);
            }
            AgentEvent::Error(msg) => {
                eprintln!("\nerror: {}", msg);
            }
        }
    }
}

pub struct SilentSink;

#[async_trait]
impl OutputSink for SilentSink {
    async fn emit(&self, _event: AgentEvent) {}
}
