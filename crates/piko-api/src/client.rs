use crate::error::ApiError;
use crate::request::MessagesRequest;
use crate::response::{ApiErrorResponse, MessagesResponse, StopReason, Usage};
use crate::stream::{
    parse_sse_line, Delta, DeltaUsage, EventStream, MessageDeltaData, MessageStartData, StreamEvent,
};
use futures_util::StreamExt;
use piko_types::message::{ContentBlock, ImageSource, ToolResultContent};
use piko_types::{Message, Role};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, warn};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApiFlavor {
    Anthropic,
    OpenAi,
}

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    use_bearer_auth: bool,
    flavor: ApiFlavor,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, ApiError> {
        Self::with_options(api_key, DEFAULT_BASE_URL, false, None)
    }

    pub fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, ApiError> {
        Self::with_options(api_key, base_url, false, None)
    }

    pub fn with_options(
        credential: impl Into<String>,
        base_url: impl Into<String>,
        use_bearer_auth: bool,
        provider: Option<&str>,
    ) -> Result<Self, ApiError> {
        let flavor = match provider.unwrap_or("anthropic") {
            "openai" => ApiFlavor::OpenAi,
            _ => ApiFlavor::Anthropic,
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if flavor == ApiFlavor::Anthropic {
            headers.insert(
                "anthropic-version",
                HeaderValue::from_static(ANTHROPIC_VERSION),
            );
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            http,
            api_key: credential.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            use_bearer_auth,
            flavor,
        })
    }

    fn anthropic_messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn openai_messages_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    fn apply_anthropic_headers(
        &self,
        builder: reqwest::RequestBuilder,
        betas: Option<&[String]>,
    ) -> reqwest::RequestBuilder {
        let mut builder = if self.use_bearer_auth {
            builder.header("Authorization", format!("Bearer {}", self.api_key))
        } else {
            builder.header("x-api-key", &self.api_key)
        };
        if let Some(betas) = betas.filter(|betas| !betas.is_empty()) {
            builder = builder.header("anthropic-beta", betas.join(","));
        }
        builder
    }

    fn apply_openai_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header("Authorization", format!("Bearer {}", self.api_key))
    }

    pub fn messages_stream(&self, request: MessagesRequest) -> EventStream {
        match self.flavor {
            ApiFlavor::Anthropic => self.messages_stream_anthropic(request),
            ApiFlavor::OpenAi => self.messages_stream_openai(request),
        }
    }

    pub async fn messages(&self, request: MessagesRequest) -> Result<MessagesResponse, ApiError> {
        match self.flavor {
            ApiFlavor::Anthropic => self.messages_anthropic(request).await,
            ApiFlavor::OpenAi => self.messages_openai(request).await,
        }
    }

    fn messages_stream_anthropic(&self, request: MessagesRequest) -> EventStream {
        let http = self.http.clone();
        let url = self.anthropic_messages_url();
        let auth_header_value = self.api_key.clone();
        let use_bearer_auth = self.use_bearer_auth;

        let stream = async_stream::try_stream! {
            let req = MessagesRequest { stream: true, ..request };

            debug!("sending messages request to {}", url);

            let messages_with_cache = req.messages_with_cache();
            let mut payload = serde_json::to_value(&req)
                .map_err(|e| ApiError::Sse(format!("request serialization: {}", e)))?;
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("messages".to_string(), serde_json::Value::Array(messages_with_cache));
            }

            let mut builder = if use_bearer_auth {
                http.post(&url).header("Authorization", format!("Bearer {auth_header_value}"))
            } else {
                http.post(&url).header("x-api-key", &auth_header_value)
            };
            if let Some(ref betas) = req.betas {
                builder = builder.header("anthropic-beta", betas.join(","));
            }
            let resp = builder.json(&payload).send().await?;

            let status = resp.status();

            if !status.is_success() {
                let retry_after = retry_after_header(&resp);
                let body = resp.text().await.unwrap_or_default();
                handle_error_status(status.as_u16(), retry_after, &body)?;
                return;
            }

            let mut bytes_stream = resp.bytes_stream();
            let mut current_event_type = String::new();
            let mut buffer = String::new();

            while let Some(chunk) = bytes_stream.next().await {
                let chunk = chunk?;
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        current_event_type.clear();
                        continue;
                    }

                    if let Some(event_type) = line.strip_prefix("event: ") {
                        current_event_type = event_type.to_string();
                    } else if let Some(data) = line.strip_prefix("data: ") {
                        match parse_sse_line(&current_event_type, data) {
                            Ok(Some(event)) => yield event,
                            Ok(None) => {}
                            Err(e) => {
                                warn!("SSE parse error: {}", e);
                                Err::<(), ApiError>(e)?;
                            }
                        }
                    }
                }
            }
        };

        Box::pin(stream)
    }

    async fn messages_anthropic(
        &self,
        request: MessagesRequest,
    ) -> Result<MessagesResponse, ApiError> {
        let req = MessagesRequest {
            stream: false,
            ..request
        };

        let req_builder = self.apply_anthropic_headers(
            self.http.post(self.anthropic_messages_url()),
            req.betas.as_deref(),
        );
        let resp = req_builder.json(&req).send().await?;

        let status = resp.status();
        let retry_after = retry_after_header(&resp);
        let body = resp.text().await?;

        if !status.is_success() {
            return handle_error_status(status.as_u16(), retry_after, &body);
        }

        let response: MessagesResponse = serde_json::from_str(&body)?;
        Ok(response)
    }

    fn messages_stream_openai(&self, request: MessagesRequest) -> EventStream {
        let http = self.http.clone();
        let url = self.openai_messages_url();
        let api_key = self.api_key.clone();

        let stream = async_stream::try_stream! {
            let req = MessagesRequest { stream: true, ..request };
            let payload = build_openai_payload(&req, true);
            let resp = http
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Accept", "text/event-stream")
                .json(&payload)
                .send()
                .await?;

            let status = resp.status();
            if !status.is_success() {
                let retry_after = retry_after_header(&resp);
                let body = resp.text().await.unwrap_or_default();
                handle_error_status(status.as_u16(), retry_after, &body)?;
                return;
            }

            let mut bytes_stream = resp.bytes_stream();
            let mut buffer = String::new();
            let mut emitted_message_start = false;
            let mut text_started = false;
            let mut tool_started: HashMap<usize, bool> = HashMap::new();
            let mut tool_ids: HashMap<usize, String> = HashMap::new();
            let mut tool_names: HashMap<usize, String> = HashMap::new();
            let mut tool_args: HashMap<usize, String> = HashMap::new();
            let mut pending_stop_reason: Option<StopReason> = None;
            let mut final_output_tokens = 0u32;
            let model_name = req.model.to_string();

            while let Some(chunk) = bytes_stream.next().await {
                let chunk = chunk?;
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };

                    if data == "[DONE]" {
                        break;
                    }

                    let json: Value = serde_json::from_str(data)
                        .map_err(|e| ApiError::Sse(format!("failed to parse openai chunk: {}", e)))?;

                    if !emitted_message_start {
                        yield StreamEvent::MessageStart {
                            message: MessageStartData {
                                id: json.get("id").and_then(|v| v.as_str()).unwrap_or("openai-stream").to_string(),
                                model: json.get("model").and_then(|v| v.as_str()).unwrap_or(&model_name).to_string(),
                                role: "assistant".to_string(),
                                usage: Usage::default(),
                            }
                        };
                        emitted_message_start = true;
                    }

                    if let Some(usage) = json.get("usage") {
                        final_output_tokens = usage
                            .get("completion_tokens")
                            .or_else(|| usage.get("output_tokens"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                    }

                    let Some(choice) = json.get("choices").and_then(|v| v.as_array()).and_then(|a| a.first()) else {
                        continue;
                    };

                    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                        pending_stop_reason = Some(map_openai_finish_reason(reason));
                    }

                    let Some(delta) = choice.get("delta").and_then(|v| v.as_object()) else {
                        continue;
                    };

                    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            if !text_started {
                                yield StreamEvent::ContentBlockStart {
                                    index: 0,
                                    content_block: ContentBlock::Text { text: String::new() },
                                };
                                text_started = true;
                            }
                            yield StreamEvent::ContentBlockDelta {
                                index: 0,
                                delta: Delta::TextDelta {
                                    text: content.to_string(),
                                },
                            };
                        }
                    }

                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tool_calls {
                            let openai_index = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            let block_index = openai_index + 1;

                            if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                tool_ids.insert(block_index, id.to_string());
                            }
                            if let Some(name) = tc
                                .get("function")
                                .and_then(|v| v.get("name"))
                                .and_then(|v| v.as_str())
                            {
                                tool_names.insert(block_index, name.to_string());
                            }
                            if let Some(arguments) = tc
                                .get("function")
                                .and_then(|v| v.get("arguments"))
                                .and_then(|v| v.as_str())
                            {
                                tool_args.entry(block_index).or_default().push_str(arguments);
                            }

                            if !tool_started.get(&block_index).copied().unwrap_or(false) {
                                if let (Some(id), Some(name)) =
                                    (tool_ids.get(&block_index), tool_names.get(&block_index))
                                {
                                    yield StreamEvent::ContentBlockStart {
                                        index: block_index,
                                        content_block: ContentBlock::ToolUse {
                                            id: id.clone(),
                                            name: name.clone(),
                                            input: json!({}),
                                        },
                                    };
                                    tool_started.insert(block_index, true);
                                    if let Some(args) = tool_args.get(&block_index).filter(|s| !s.is_empty()) {
                                        yield StreamEvent::ContentBlockDelta {
                                            index: block_index,
                                            delta: Delta::InputJsonDelta {
                                                partial_json: args.clone(),
                                            },
                                        };
                                    }
                                }
                            } else if let Some(arguments) = tc
                                .get("function")
                                .and_then(|v| v.get("arguments"))
                                .and_then(|v| v.as_str())
                            {
                                if !arguments.is_empty() {
                                    yield StreamEvent::ContentBlockDelta {
                                        index: block_index,
                                        delta: Delta::InputJsonDelta {
                                            partial_json: arguments.to_string(),
                                        },
                                    };
                                }
                            }
                        }
                    }
                }
            }

            for index in tool_started.keys().copied().collect::<Vec<_>>() {
                yield StreamEvent::ContentBlockStop { index };
            }

            yield StreamEvent::MessageDelta {
                delta: MessageDeltaData {
                    stop_reason: pending_stop_reason,
                    stop_sequence: None,
                },
                usage: Some(DeltaUsage {
                    output_tokens: final_output_tokens,
                }),
            };
            yield StreamEvent::MessageStop;
        };

        Box::pin(stream)
    }

    async fn messages_openai(
        &self,
        request: MessagesRequest,
    ) -> Result<MessagesResponse, ApiError> {
        let payload = build_openai_payload(&request, false);
        let resp = self
            .apply_openai_headers(self.http.post(self.openai_messages_url()))
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        let retry_after = retry_after_header(&resp);
        let body = resp.text().await?;

        if !status.is_success() {
            return handle_error_status(status.as_u16(), retry_after, &body);
        }

        let json: Value = serde_json::from_str(&body)?;
        parse_openai_messages_response(&json, &request.model.to_string())
    }
}

fn retry_after_header(resp: &reqwest::Response) -> Option<u64> {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

fn handle_error_status<T>(
    status: u16,
    retry_after: Option<u64>,
    body: &str,
) -> Result<T, ApiError> {
    if status == 429 {
        return Err(ApiError::RateLimit { retry_after });
    }
    if status == 529 {
        return Err(ApiError::Overloaded);
    }
    if status == 401 {
        return Err(ApiError::Auth(body.to_string()));
    }
    let message = serde_json::from_str::<ApiErrorResponse>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string());
    Err(ApiError::ApiResponse { status, message })
}

fn build_openai_payload(request: &MessagesRequest, stream: bool) -> Value {
    let mut body = json!({
        "model": request.model.to_string(),
        "max_tokens": request.max_tokens,
        "messages": to_openai_messages(&request.messages, request.system.as_ref()),
        "stream": stream,
    });

    if stream {
        body["stream_options"] = json!({ "include_usage": true });
    }

    if !request.tools.is_empty() {
        body["tools"] = json!(to_openai_tools(&request.tools));
    }

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    body
}

fn to_openai_messages(
    messages: &[Message],
    system: Option<&Vec<crate::request::SystemBlock>>,
) -> Vec<Value> {
    let mut out = Vec::new();

    if let Some(system_blocks) = system {
        let text = system_blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        out.push(json!({ "role": "system", "content": text }));
    }

    for msg in messages {
        match msg.role {
            Role::User => append_user_message(&mut out, msg),
            Role::Assistant => append_assistant_message(&mut out, msg),
        }
    }

    out
}

fn append_user_message(out: &mut Vec<Value>, msg: &Message) {
    let mut text_parts = Vec::new();
    let mut content_parts = Vec::new();
    let mut has_rich_content = false;

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
                content_parts.push(json!({
                    "type": "text",
                    "text": text,
                }));
            }
            ContentBlock::Image { source } => {
                has_rich_content = true;
                if let Some(part) = openai_image_part(source) {
                    content_parts.push(part);
                }
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let text = match content {
                    ToolResultContent::Text(t) => t.clone(),
                    ToolResultContent::Blocks(blocks) => blocks
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": text,
                }));
            }
            _ => {}
        }
    }

    if has_rich_content {
        if !content_parts.is_empty() {
            out.push(json!({
                "role": "user",
                "content": content_parts,
            }));
        }
    } else if !text_parts.is_empty() {
        out.push(json!({
            "role": "user",
            "content": text_parts.join(""),
        }));
    }
}

fn openai_image_part(source: &ImageSource) -> Option<Value> {
    match source {
        ImageSource::Base64 { media_type, data } => Some(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", media_type, data),
            }
        })),
        ImageSource::Url { url } => Some(json!({
            "type": "image_url",
            "image_url": {
                "url": url,
            }
        })),
    }
}

fn append_assistant_message(out: &mut Vec<Value>, msg: &Message) {
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => text_parts.push(text.clone()),
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                    }
                }));
            }
            _ => {}
        }
    }

    let content = if text_parts.is_empty() {
        Value::Null
    } else {
        Value::String(text_parts.join(""))
    };

    let mut obj = serde_json::Map::new();
    obj.insert("role".to_string(), json!("assistant"));
    obj.insert("content".to_string(), content);
    if !tool_calls.is_empty() {
        obj.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    out.push(Value::Object(obj));
}

fn to_openai_tools(tools: &[Value]) -> Vec<Value> {
    tools.iter().filter_map(|tool| {
        let obj = tool.as_object()?;
        Some(json!({
            "type": "function",
            "function": {
                "name": obj.get("name").cloned().unwrap_or_else(|| json!("")),
                "description": obj.get("description").cloned().unwrap_or_else(|| json!("")),
                "parameters": obj.get("input_schema").cloned().unwrap_or_else(|| json!({ "type": "object", "properties": {}, "required": [] })),
            }
        }))
    }).collect()
}

fn parse_openai_messages_response(
    json: &Value,
    fallback_model: &str,
) -> Result<MessagesResponse, ApiError> {
    let id = json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("openai")
        .to_string();
    let model = json
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_model)
        .to_string();
    let choice = json
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| ApiError::ApiResponse {
            status: 500,
            message: "No choices in OpenAI response".to_string(),
        })?;
    let message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
    let mut content = Vec::new();

    if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            content.push(ContentBlock::Text {
                text: text.to_string(),
            });
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for tc in tool_calls {
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = tc
                .get("function")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input = tc
                .get("function")
                .and_then(|v| v.get("arguments"))
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| json!({}));
            content.push(ContentBlock::ToolUse { id, name, input });
        }
    }

    let usage_json = json.get("usage");
    let usage = Usage {
        input_tokens: usage_json
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage_json
            .and_then(|u| {
                u.get("completion_tokens")
                    .or_else(|| u.get("output_tokens"))
            })
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    };

    Ok(MessagesResponse {
        id,
        model,
        role: "assistant".to_string(),
        content,
        stop_reason: choice
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .map(map_openai_finish_reason),
        usage,
    })
}

fn map_openai_finish_reason(reason: &str) -> StopReason {
    match reason {
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "stop" | "content_filter" => StopReason::EndTurn,
        _ => StopReason::EndTurn,
    }
}
