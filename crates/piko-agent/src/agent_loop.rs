use crate::agent::AgentConfig;
use crate::context::ConversationContext;
use crate::output::{AgentEvent, OutputSink};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use piko_api::stream::{Delta, StreamEvent};
use piko_api::AnthropicClient;
use piko_api::request::MessagesRequest;
use piko_permissions::checker::{PermissionChecker, PermissionDecision, PermissionRequest};
use piko_tools::registry::ToolRegistry;
use piko_tools::tool_trait::ToolContext;
use piko_types::message::ContentBlock;
use piko_types::tool::{ToolCall, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

pub async fn run_turn(
    client: &AnthropicClient,
    tools: &ToolRegistry,
    permissions: &dyn PermissionChecker,
    context: &mut ConversationContext,
    config: &AgentConfig,
    sink: Arc<dyn OutputSink>,
    cancellation: CancellationToken,
) -> Result<String> {
    let max_turns = config.max_turns.unwrap_or(50);
    let mut turns = 0;
    let mut final_text = String::new();

    loop {
        if turns >= max_turns {
            sink.emit(AgentEvent::Error(format!("reached max turns limit ({})", max_turns))).await;
            break;
        }

        if cancellation.is_cancelled() {
            break;
        }

        let mut request = MessagesRequest::new(config.model.clone(), context.messages.to_vec())
            .with_max_tokens(config.max_tokens)
            .with_tools(tools.definitions());

        if let Some(ref system) = context.system_prompt {
            request = request.with_system(system.clone());
        }

        let mut stream = client.messages_stream(request);

        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut tool_input_buffers: HashMap<usize, String> = HashMap::new();
        let mut tool_names: HashMap<usize, (String, String)> = HashMap::new();
        let mut stop_reason = None;
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;
        let mut current_text = String::new();

        while let Some(event_result) = stream.next().await {
            if cancellation.is_cancelled() {
                return Err(anyhow!("cancelled"));
            }

            let event = match event_result {
                Ok(e) => e,
                Err(e) => {
                    sink.emit(AgentEvent::Error(e.to_string())).await;
                    return Err(e.into());
                }
            };

            match event {
                StreamEvent::MessageStart { message } => {
                    input_tokens = message.usage.input_tokens;
                }
                StreamEvent::ContentBlockStart { index, content_block } => {
                    match &content_block {
                        ContentBlock::ToolUse { id, name, .. } => {
                            tool_names.insert(index, (id.clone(), name.clone()));
                            tool_input_buffers.insert(index, String::new());
                        }
                        ContentBlock::Text { text } => {
                            if !text.is_empty() {
                                sink.emit(AgentEvent::TextChunk(text.clone())).await;
                                current_text.push_str(text);
                            }
                        }
                        _ => {}
                    }
                    while content_blocks.len() <= index {
                        content_blocks.push(ContentBlock::Text { text: String::new() });
                    }
                    content_blocks[index] = content_block;
                }
                StreamEvent::ContentBlockDelta { index, delta } => {
                    match delta {
                        Delta::TextDelta { text } => {
                            sink.emit(AgentEvent::TextChunk(text.clone())).await;
                            current_text.push_str(&text);
                            if let Some(ContentBlock::Text { text: existing }) = content_blocks.get_mut(index) {
                                existing.push_str(&text);
                            }
                        }
                        Delta::InputJsonDelta { partial_json } => {
                            if let Some(buf) = tool_input_buffers.get_mut(&index) {
                                buf.push_str(&partial_json);
                            }
                        }
                    }
                }
                StreamEvent::ContentBlockStop { index } => {
                    if let Some(buf) = tool_input_buffers.remove(&index) {
                        if let Some((id, name)) = tool_names.remove(&index) {
                            let input: serde_json::Value = serde_json::from_str(&buf)
                                .unwrap_or(serde_json::Value::Object(Default::default()));
                            if let Some(block) = content_blocks.get_mut(index) {
                                *block = ContentBlock::ToolUse { id, name, input };
                            }
                        }
                    }
                }
                StreamEvent::MessageDelta { delta, usage } => {
                    stop_reason = delta.stop_reason;
                    if let Some(u) = usage {
                        output_tokens = u.output_tokens;
                    }
                }
                StreamEvent::MessageStop => {
                    break;
                }
                StreamEvent::Ping => {}
                StreamEvent::Error { error } => {
                    sink.emit(AgentEvent::Error(error.message.clone())).await;
                    return Err(anyhow!("API stream error: {}", error.message));
                }
            }
        }

        if !current_text.is_empty() {
            final_text = current_text;
        }

        context.push_assistant_blocks(content_blocks.clone());

        sink.emit(AgentEvent::TurnComplete { input_tokens, output_tokens }).await;

        let tool_calls: Vec<ToolCall> = content_blocks
            .iter()
            .filter_map(|b| {
                if let ContentBlock::ToolUse { id, name, input } = b {
                    Some(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        if tool_calls.is_empty() {
            break;
        }

        let tool_ctx = ToolContext::new(config.cwd.clone());
        let mut tool_results: Vec<ToolResult> = Vec::new();

        for call in &tool_calls {
            sink.emit(AgentEvent::ToolCallStarted(call.clone())).await;

            let tool = match tools.get(&call.name) {
                Some(t) => t,
                None => {
                    let result = ToolResult::error(call.id.clone(), format!("unknown tool: {}", call.name));
                    sink.emit(AgentEvent::ToolCallCompleted { call: call.clone(), result: result.clone() }).await;
                    tool_results.push(result);
                    continue;
                }
            };

            let perm_request = PermissionRequest {
                tool_name: call.name.clone(),
                description: tool.description_for_permission(&call.input),
                input: call.input.clone(),
            };

            let decision = permissions.check(&perm_request).await;

            let result = match decision {
                PermissionDecision::Deny | PermissionDecision::DenyAlways => {
                    ToolResult::error(call.id.clone(), "permission denied by user".to_string())
                }
                PermissionDecision::Allow | PermissionDecision::AllowAlways => {
                    let mut input_with_id = call.input.clone();
                    if let Some(obj) = input_with_id.as_object_mut() {
                        obj.insert("__tool_use_id".to_string(), serde_json::Value::String(call.id.clone()));
                    }
                    tool.execute(input_with_id, &tool_ctx).await
                }
            };

            sink.emit(AgentEvent::ToolCallCompleted { call: call.clone(), result: result.clone() }).await;
            tool_results.push(result);
        }

        context.push_tool_results(tool_results);
        turns += 1;
    }

    Ok(final_text)
}
