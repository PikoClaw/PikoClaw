use crate::agent::AgentConfig;
use crate::context::ConversationContext;
use crate::output::{AgentEvent, OutputSink};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use piko_api::error::ApiError;
use piko_api::request::MessagesRequest;
use piko_api::stream::{Delta, StreamEvent};
use piko_api::AnthropicClient;
use piko_permissions::checker::{PermissionChecker, PermissionDecision, PermissionRequest};
use piko_tools::registry::ToolRegistry;
use piko_tools::tool_trait::ToolContext;
use piko_types::message::ContentBlock;
use piko_types::tool::{ToolCall, ToolResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const PLAN_MODE_BLOCKED: &[&str] = &["bash", "file_write", "file_edit", "notebook_edit"];

#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    client: &AnthropicClient,
    tools: &ToolRegistry,
    permissions: &dyn PermissionChecker,
    context: &mut ConversationContext,
    config: &AgentConfig,
    sink: Arc<dyn OutputSink>,
    cancellation: CancellationToken,
    plan_mode: Arc<AtomicBool>,
) -> Result<String> {
    let max_turns = config.max_turns.unwrap_or(50);
    let mut turns = 0;
    let mut final_text = String::new();

    loop {
        if turns >= max_turns {
            sink.emit(AgentEvent::Error(format!(
                "reached max turns limit ({})",
                max_turns
            )))
            .await;
            break;
        }

        if cancellation.is_cancelled() {
            break;
        }

        let mut request = MessagesRequest::new(config.model.clone(), context.messages.to_vec())
            .with_max_tokens(config.max_tokens)
            .with_tools(tools.definitions());

        if tools.has_web_search() {
            request = request
                .with_raw_tool(serde_json::json!({
                    "type": "web_search_20250305",
                    "name": "web_search"
                }))
                .with_betas(vec!["web-search-2025-03-05".to_string()]);
        }

        if config.extended_thinking {
            let budget = config.thinking_budget_tokens;
            let needed_max = budget.saturating_add(2048);
            if needed_max > request.max_tokens {
                request = request.with_max_tokens(needed_max);
            }
            request = request.with_thinking(budget);
        }

        if let Some(ref system) = context.system_prompt {
            request = request.with_system(system.clone());
        }

        let mut stream = client.messages_stream(request);

        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut tool_input_buffers: HashMap<usize, String> = HashMap::new();
        let mut tool_names: HashMap<usize, (String, String)> = HashMap::new();
        let mut _stop_reason = None;
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;
        let mut cache_creation_tokens = 0u32;
        let mut cache_read_tokens = 0u32;
        let mut current_text = String::new();

        while let Some(event_result) = stream.next().await {
            if cancellation.is_cancelled() {
                return Err(anyhow!("cancelled"));
            }

            let event = match event_result {
                Ok(e) => e,
                Err(e) => {
                    if let ApiError::RateLimit { retry_after } = &e {
                        sink.emit(AgentEvent::RateLimit {
                            retry_after: *retry_after,
                        })
                        .await;
                    } else {
                        sink.emit(AgentEvent::Error(e.to_string())).await;
                    }
                    return Err(e.into());
                }
            };

            match event {
                StreamEvent::MessageStart { message } => {
                    input_tokens = message.usage.input_tokens;
                    cache_creation_tokens = message.usage.cache_creation_input_tokens;
                    cache_read_tokens = message.usage.cache_read_input_tokens;
                }
                StreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => {
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
                        content_blocks.push(ContentBlock::Text {
                            text: String::new(),
                        });
                    }
                    content_blocks[index] = content_block;
                }
                StreamEvent::ContentBlockDelta { index, delta } => match delta {
                    Delta::TextDelta { text } => {
                        sink.emit(AgentEvent::TextChunk(text.clone())).await;
                        current_text.push_str(&text);
                        if let Some(ContentBlock::Text { text: existing }) =
                            content_blocks.get_mut(index)
                        {
                            existing.push_str(&text);
                        }
                    }
                    Delta::InputJsonDelta { partial_json } => {
                        if let Some(buf) = tool_input_buffers.get_mut(&index) {
                            buf.push_str(&partial_json);
                        }
                    }
                    Delta::ThinkingDelta { thinking } => {
                        sink.emit(AgentEvent::ThinkingChunk(thinking.clone())).await;
                        if let Some(ContentBlock::Thinking { thinking: existing }) =
                            content_blocks.get_mut(index)
                        {
                            existing.push_str(&thinking);
                        }
                    }
                },
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
                    _stop_reason = delta.stop_reason;
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

        let history_blocks: Vec<ContentBlock> = content_blocks
            .iter()
            .filter(|b| !b.is_thinking())
            .cloned()
            .collect();
        context.push_assistant_blocks(history_blocks);

        sink.emit(AgentEvent::TurnComplete {
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        })
        .await;

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

            if plan_mode.load(Ordering::SeqCst) && PLAN_MODE_BLOCKED.contains(&call.name.as_str()) {
                let result = ToolResult::error(
                    call.id.clone(),
                    format!(
                        "Tool '{}' is blocked in plan mode. Call exit_plan_mode first.",
                        call.name
                    ),
                );
                sink.emit(AgentEvent::ToolCallCompleted {
                    call: call.clone(),
                    result: result.clone(),
                })
                .await;
                tool_results.push(result);
                continue;
            }

            let tool = match tools.get(&call.name) {
                Some(t) => t,
                None => {
                    let result =
                        ToolResult::error(call.id.clone(), format!("unknown tool: {}", call.name));
                    sink.emit(AgentEvent::ToolCallCompleted {
                        call: call.clone(),
                        result: result.clone(),
                    })
                    .await;
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
                        obj.insert(
                            "__tool_use_id".to_string(),
                            serde_json::Value::String(call.id.clone()),
                        );
                    }
                    tool.execute(input_with_id, &tool_ctx).await
                }
            };

            sink.emit(AgentEvent::ToolCallCompleted {
                call: call.clone(),
                result: result.clone(),
            })
            .await;
            tool_results.push(result);
        }

        context.push_tool_results(tool_results);
        turns += 1;
    }

    Ok(final_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_mode_blocked_contains_mutating_tools() {
        assert!(PLAN_MODE_BLOCKED.contains(&"bash"));
        assert!(PLAN_MODE_BLOCKED.contains(&"file_write"));
        assert!(PLAN_MODE_BLOCKED.contains(&"file_edit"));
        assert!(PLAN_MODE_BLOCKED.contains(&"notebook_edit"));
    }

    #[test]
    fn plan_mode_blocked_does_not_contain_read_only_tools() {
        assert!(!PLAN_MODE_BLOCKED.contains(&"file_read"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"glob"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"grep"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"web_fetch"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"web_search"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"AskUserQuestion"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"enter_plan_mode"));
        assert!(!PLAN_MODE_BLOCKED.contains(&"exit_plan_mode"));
    }

    #[test]
    fn plan_mode_flag_starts_false_by_default() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn plan_mode_flag_can_be_toggled() {
        let flag = Arc::new(AtomicBool::new(false));
        flag.store(true, Ordering::SeqCst);
        assert!(flag.load(Ordering::SeqCst));
        flag.store(false, Ordering::SeqCst);
        assert!(!flag.load(Ordering::SeqCst));
    }
}
