// google.rs — GoogleClient: Google Gemini API client for PikoClaw.
//
// Supports:
// - Non-streaming: POST .../generateContent?key={api_key}
// - Streaming SSE: POST .../streamGenerateContent?alt=sse&key={api_key}
// - Tool/function calling via functionDeclarations
// - System prompts via systemInstruction field
// - Thinking config for Gemini 2.5+ models
// - Image inputs via inlineData parts

use crate::error::ApiError;
use crate::request::MessagesRequest;
use crate::response::{MessagesResponse, StopReason, Usage};
use crate::stream::{Delta, DeltaUsage, EventStream, MessageDeltaData, MessageStartData, StreamEvent};
use futures_util::StreamExt;
use piko_types::message::{ContentBlock, ImageSource, ToolResultContent};
use piko_types::Role;
use serde_json::{json, Value};
use tracing::{debug, warn};

const GOOGLE_BASE_URL: &str = "https://generativelanguage.googleapis.com";

pub struct GoogleClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl GoogleClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: GOOGLE_BASE_URL.to_string(),
        }
    }

    fn generate_url(&self, model: &str) -> String {
        format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base_url, model, self.api_key
        )
    }

    fn supports_thinking(model: &str) -> bool {
        model.contains("2.5") || model.contains("3.0") || model.contains("3.1")
            || model.contains("gemini-3")
    }

    fn tool_use_id_for_name(name: &str, occurrence: usize) -> String {
        let sanitized: String = name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' { ch } else { '_' })
            .collect();
        let base = if sanitized.is_empty() { "tool" } else { sanitized.as_str() };
        if occurrence == 0 {
            format!("call_{}", base)
        } else {
            format!("call_{}_{}", base, occurrence + 1)
        }
    }

    fn tool_name_by_id(messages: &[piko_types::Message]) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        for msg in messages {
            for block in &msg.content {
                if let ContentBlock::ToolUse { id, name, .. } = block {
                    map.insert(id.clone(), name.clone());
                }
            }
        }
        map
    }

    fn infer_tool_name_from_id(tool_use_id: &str) -> Option<String> {
        let raw = tool_use_id.strip_prefix("call_")?;
        let trimmed = if let Some((candidate, suffix)) = raw.rsplit_once('_') {
            if !candidate.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
                candidate
            } else {
                raw
            }
        } else {
            raw
        };
        if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
    }

    fn content_block_to_part(block: &ContentBlock) -> Option<Value> {
        match block {
            ContentBlock::Text { text } => Some(json!({ "text": text })),

            ContentBlock::Image { source } => match source {
                ImageSource::Base64 { media_type, data } => Some(json!({
                    "inlineData": { "data": data, "mimeType": media_type }
                })),
                ImageSource::Url { url } => Some(json!({
                    "fileData": {
                        "fileUri": url,
                        "mimeType": "image/jpeg"
                    }
                })),
            },

            ContentBlock::ToolUse { name, input, .. } => Some(json!({
                "functionCall": { "name": name, "args": input }
            })),

            ContentBlock::Thinking { .. } => None,

            ContentBlock::ToolResult { .. }
            | ContentBlock::ServerToolUse { .. }
            | ContentBlock::ServerToolResult { .. }
            | ContentBlock::Unknown => None,
        }
    }

    fn tool_result_to_part(tool_name: &str, content: &ToolResultContent) -> Value {
        let response_content = match content {
            ToolResultContent::Text(t) => json!({ "content": t }),
            ToolResultContent::Blocks(blocks) => {
                let text: String = blocks
                    .iter()
                    .filter_map(|b| if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                    .collect::<Vec<_>>()
                    .join("\n");
                json!({ "content": text })
            }
        };
        json!({
            "functionResponse": {
                "name": tool_name,
                "response": response_content
            }
        })
    }

    fn sanitize_schema(schema: Value) -> Value {
        match schema {
            Value::Object(mut map) => {
                map.remove("additionalProperties");
                map.remove("$schema");
                map.remove("default");
                map.remove("examples");
                map.remove("title");

                let schema_type = map
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let Some(Value::Array(enum_vals)) = map.get("enum") {
                    if enum_vals.iter().any(|v| v.is_number()) {
                        let string_enums: Vec<Value> =
                            enum_vals.iter().map(|v| Value::String(v.to_string())).collect();
                        map.insert("enum".to_string(), Value::Array(string_enums));
                        map.insert("type".to_string(), Value::String("string".to_string()));
                    }
                }

                if schema_type.as_deref() == Some("object") {
                    if let Some(Value::Object(props)) = map.get_mut("properties") {
                        let sanitized_props: serde_json::Map<String, Value> = props
                            .iter()
                            .map(|(k, v)| (k.clone(), Self::sanitize_schema(v.clone())))
                            .collect();
                        *props = sanitized_props;
                    }

                    if let Some(required) = map.get("required").cloned() {
                        if let Value::Array(req_arr) = required {
                            let prop_keys: std::collections::HashSet<String> = map
                                .get("properties")
                                .and_then(|p| p.as_object())
                                .map(|o| o.keys().cloned().collect())
                                .unwrap_or_default();
                            let filtered: Vec<Value> = req_arr
                                .into_iter()
                                .filter(|v| v.as_str().map(|s| prop_keys.contains(s)).unwrap_or(false))
                                .collect();
                            map.insert("required".to_string(), Value::Array(filtered));
                        }
                    }
                } else {
                    map.remove("properties");
                    map.remove("required");
                }

                if schema_type.as_deref() == Some("array") {
                    if let Some(items) = map.get_mut("items") {
                        if let Value::Object(ref mut items_map) = items {
                            if !items_map.contains_key("type") {
                                items_map.insert("type".to_string(), Value::String("string".to_string()));
                            }
                            let sanitized = Self::sanitize_schema(Value::Object(items_map.clone()));
                            *items = sanitized;
                        }
                    }
                }

                Value::Object(map)
            }
            other => other,
        }
    }

    fn build_request_body(&self, request: &MessagesRequest) -> Value {
        let tool_name_by_id = Self::tool_name_by_id(&request.messages);
        let mut contents: Vec<Value> = Vec::new();

        for msg in &request.messages {
            let role = match msg.role {
                Role::User => "user",
                Role::Assistant => "model",
            };

            let mut regular_parts: Vec<Value> = Vec::new();
            let mut tool_result_parts: Vec<Value> = Vec::new();

            for block in &msg.content {
                if let ContentBlock::ToolResult { tool_use_id, content, .. } = block {
                    if !regular_parts.is_empty() {
                        contents.push(json!({ "role": role, "parts": regular_parts.drain(..).collect::<Vec<_>>() }));
                    }
                    let tool_name = tool_name_by_id
                        .get(tool_use_id)
                        .cloned()
                        .or_else(|| Self::infer_tool_name_from_id(tool_use_id))
                        .unwrap_or_else(|| tool_use_id.clone());
                    tool_result_parts.push(Self::tool_result_to_part(&tool_name, content));
                } else if let Some(part) = Self::content_block_to_part(block) {
                    if !tool_result_parts.is_empty() {
                        contents.push(json!({ "role": "user", "parts": tool_result_parts.drain(..).collect::<Vec<_>>() }));
                    }
                    regular_parts.push(part);
                }
            }

            if !regular_parts.is_empty() {
                contents.push(json!({ "role": role, "parts": regular_parts }));
            }
            if !tool_result_parts.is_empty() {
                contents.push(json!({ "role": "user", "parts": tool_result_parts }));
            }
        }

        let system_instruction: Option<Value> = request.system.as_ref().map(|blocks| {
            let text = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
            json!({ "parts": [{ "text": text }] })
        });

        let tools_value: Option<Value> = if request.tools.is_empty() {
            None
        } else {
            let declarations: Vec<Value> = request.tools.iter().filter_map(|tool| {
                let obj = tool.as_object()?;
                Some(json!({
                    "name": obj.get("name").cloned().unwrap_or_else(|| json!("")),
                    "description": obj.get("description").cloned().unwrap_or_else(|| json!("")),
                    "parameters": Self::sanitize_schema(
                        obj.get("input_schema").cloned().unwrap_or_else(|| json!({ "type": "object", "properties": {} }))
                    )
                }))
            }).collect();
            Some(json!([{ "functionDeclarations": declarations }]))
        };

        let mut gen_config = serde_json::Map::new();
        gen_config.insert("maxOutputTokens".to_string(), json!(request.max_tokens));
        if let Some(temp) = request.temperature {
            gen_config.insert("temperature".to_string(), json!(temp));
        }
        if let Some(thinking) = &request.thinking {
            let model_str = request.model.to_string();
            if Self::supports_thinking(&model_str) {
                gen_config.insert("thinkingConfig".to_string(), json!({
                    "includeThoughts": true,
                    "thinkingBudget": thinking.budget_tokens
                }));
            }
        }

        let mut body = serde_json::Map::new();
        body.insert("contents".to_string(), Value::Array(contents));
        body.insert("generationConfig".to_string(), Value::Object(gen_config));
        if let Some(si) = system_instruction {
            body.insert("systemInstruction".to_string(), si);
        }
        if let Some(tools) = tools_value {
            body.insert("tools".to_string(), tools);
        }

        Value::Object(body)
    }

    fn parse_finish_reason(reason: &str) -> StopReason {
        match reason {
            "MAX_TOKENS" => StopReason::MaxTokens,
            "TOOL_CODE" | "FUNCTION_CALL" => StopReason::ToolUse,
            _ => StopReason::EndTurn,
        }
    }

    fn parse_response_body(body: &Value, model: &str) -> Result<MessagesResponse, ApiError> {
        let candidates = body
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| ApiError::ApiResponse {
                status: 500,
                message: "Missing 'candidates' in Google response".to_string(),
            })?;

        let candidate = candidates.first().ok_or_else(|| ApiError::ApiResponse {
            status: 500,
            message: "Empty 'candidates' array in Google response".to_string(),
        })?;

        let finish_reason = candidate
            .get("finishReason")
            .and_then(|r| r.as_str())
            .unwrap_or("STOP");

        let stop_reason = Some(Self::parse_finish_reason(finish_reason));

        let parts = candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());

        let mut content: Vec<ContentBlock> = Vec::new();
        let mut tool_name_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        if let Some(parts) = parts {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        content.push(ContentBlock::Text { text: text.to_string() });
                    }
                } else if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    let occurrence = tool_name_counts.entry(name.clone()).and_modify(|c| *c += 1).or_insert(0);
                    let id = Self::tool_use_id_for_name(&name, *occurrence);
                    content.push(ContentBlock::ToolUse { id, name, input: args });
                }
            }
        }

        let usage = extract_usage(body);

        Ok(MessagesResponse {
            id: format!("gemini-{}", uuid::Uuid::new_v4().simple()),
            model: model.to_string(),
            role: "assistant".to_string(),
            content,
            stop_reason,
            usage,
        })
    }

    pub async fn messages(&self, request: MessagesRequest) -> Result<MessagesResponse, ApiError> {
        let model = request.model.to_string();
        let url = self.generate_url(&model);
        let body = self.build_request_body(&request);

        debug!("Google messages: POST {}", url);

        let resp = self
            .http
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let resp_body = resp.text().await?;

        if status >= 400 {
            return Err(google_error(status, &resp_body));
        }

        let json_body: Value = serde_json::from_str(&resp_body)?;
        Self::parse_response_body(&json_body, &model)
    }

    pub fn messages_stream(&self, request: MessagesRequest) -> EventStream {
        let http = self.http.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        let stream = async_stream::try_stream! {
            let model = request.model.to_string();
            let url = format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
                base_url, model, api_key
            );
            let body = {
                let client = GoogleClient {
                    http: http.clone(),
                    api_key: api_key.clone(),
                    base_url: base_url.clone(),
                };
                client.build_request_body(&request)
            };

            debug!("Google messages_stream: POST {}", url);

            let resp = http
                .post(&url)
                .header("x-goog-api-key", &api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await?;

            let status = resp.status().as_u16();
            if status >= 400 {
                let resp_body = resp.text().await.unwrap_or_default();
                Err::<(), ApiError>(google_error(status, &resp_body))?;
                return;
            }

            let message_id = format!("gemini-{}", uuid::Uuid::new_v4().simple());
            let mut emitted_message_start = false;
            let mut text_started = false;
            let mut next_tool_index: usize = 1;
            let mut tool_name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            let mut line_buf = String::new();
            let mut bytes_stream = resp.bytes_stream();
            let mut final_usage: Option<Usage> = None;
            let mut pending_stop_reason: Option<StopReason> = None;

            while let Some(chunk_result) = bytes_stream.next().await {
                let chunk = chunk_result?;
                let chunk_str = match std::str::from_utf8(&chunk) {
                    Ok(s) => s,
                    Err(_) => {
                        warn!("Google SSE: non-UTF8 chunk, skipping");
                        continue;
                    }
                };

                line_buf.push_str(chunk_str);

                while let Some(newline_pos) = line_buf.find('\n') {
                    let line = line_buf[..newline_pos].trim_end_matches('\r').to_string();
                    line_buf = line_buf[newline_pos + 1..].to_string();

                    let Some(data) = line.strip_prefix("data: ") else { continue };
                    let data = data.trim();
                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Google SSE: JSON parse error: {}: {}", e, &data[..data.len().min(120)]);
                            continue;
                        }
                    };

                    if let Some(err) = parsed.get("error") {
                        let msg = err
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown Google API error")
                            .to_string();
                        Err::<(), ApiError>(ApiError::ApiResponse { status: 500, message: msg })?;
                        return;
                    }

                    if !emitted_message_start {
                        emitted_message_start = true;
                        yield StreamEvent::MessageStart {
                            message: MessageStartData {
                                id: message_id.clone(),
                                model: model.clone(),
                                role: "assistant".to_string(),
                                usage: Usage::default(),
                            }
                        };
                    }

                    let usage = extract_usage(&parsed);
                    if usage.input_tokens > 0 || usage.output_tokens > 0 {
                        final_usage = Some(usage);
                    }

                    let candidates = parsed.get("candidates").and_then(|c| c.as_array());
                    let Some(candidates) = candidates else { continue };

                    for candidate in candidates {
                        if let Some(reason) = candidate.get("finishReason").and_then(|r| r.as_str()) {
                            if reason != "STOP" || pending_stop_reason.is_none() {
                                pending_stop_reason = Some(GoogleClient::parse_finish_reason(reason));
                            }
                        }

                        let parts = candidate
                            .get("content")
                            .and_then(|c| c.get("parts"))
                            .and_then(|p| p.as_array());

                        let Some(parts) = parts else { continue };

                        for part in parts {
                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                if !text.is_empty() {
                                    if !text_started {
                                        text_started = true;
                                        yield StreamEvent::ContentBlockStart {
                                            index: 0,
                                            content_block: ContentBlock::Text { text: String::new() },
                                        };
                                    }
                                    yield StreamEvent::ContentBlockDelta {
                                        index: 0,
                                        delta: Delta::TextDelta { text: text.to_string() },
                                    };
                                }
                            } else if let Some(fc) = part.get("functionCall") {
                                let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                let args = fc.get("args").cloned().unwrap_or(json!({}));
                                let occurrence = tool_name_counts.entry(name.clone()).and_modify(|c| *c += 1).or_insert(0);
                                let id = GoogleClient::tool_use_id_for_name(&name, *occurrence);
                                let tool_index = next_tool_index;
                                next_tool_index += 1;

                                yield StreamEvent::ContentBlockStart {
                                    index: tool_index,
                                    content_block: ContentBlock::ToolUse {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: json!({}),
                                    },
                                };
                                let args_str = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                                yield StreamEvent::ContentBlockDelta {
                                    index: tool_index,
                                    delta: Delta::InputJsonDelta { partial_json: args_str },
                                };
                                yield StreamEvent::ContentBlockStop { index: tool_index };
                            }
                        }
                    }
                }
            }

            if text_started {
                yield StreamEvent::ContentBlockStop { index: 0 };
            }

            let (input_tokens, output_tokens) = final_usage
                .map(|u| (u.input_tokens, u.output_tokens))
                .unwrap_or((0, 0));

            yield StreamEvent::MessageDelta {
                delta: MessageDeltaData {
                    stop_reason: pending_stop_reason,
                    stop_sequence: None,
                },
                usage: Some(DeltaUsage { input_tokens, output_tokens }),
            };
            yield StreamEvent::MessageStop;
        };

        Box::pin(stream)
    }
}

fn extract_usage(body: &Value) -> Usage {
    let meta = body.get("usageMetadata");
    Usage {
        input_tokens: meta
            .and_then(|m| m.get("promptTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: meta
            .and_then(|m| m.get("candidatesTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    }
}

fn google_error(status: u16, body: &str) -> ApiError {
    if status == 429 {
        return ApiError::RateLimit { retry_after: None };
    }
    if status == 401 || status == 403 {
        return ApiError::Auth(body.to_string());
    }
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| body.to_string());
    ApiError::ApiResponse { status, message }
}
