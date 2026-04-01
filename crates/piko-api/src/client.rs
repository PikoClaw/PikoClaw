use crate::error::ApiError;
use crate::request::MessagesRequest;
use crate::response::{ApiErrorResponse, MessagesResponse};
use crate::stream::{parse_sse_line, EventStream, StreamEvent};
use futures_util::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::pin::Pin;
use tracing::{debug, warn};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, ApiError> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Result<Self, ApiError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static(ANTHROPIC_VERSION));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            http,
            api_key: api_key.into(),
            base_url: base_url.into(),
        })
    }

    pub fn messages_stream(&self, request: MessagesRequest) -> EventStream {
        let http = self.http.clone();
        let api_key = self.api_key.clone();
        let url = format!("{}/v1/messages", self.base_url);

        let stream = async_stream::try_stream! {
            let mut req = MessagesRequest { stream: true, ..request };
            req.stream = true;

            debug!("sending messages request to {}", url);

            let resp = http
                .post(&url)
                .header("x-api-key", &api_key)
                .json(&req)
                .send()
                .await?;

            let status = resp.status();

            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                if status.as_u16() == 429 {
                    Err(ApiError::RateLimit { retry_after: None })?;
                } else if status.as_u16() == 529 {
                    Err(ApiError::Overloaded)?;
                } else if status.as_u16() == 401 {
                    Err(ApiError::Auth(body))?;
                } else {
                    let message = serde_json::from_str::<ApiErrorResponse>(&body)
                        .map(|e| e.error.message)
                        .unwrap_or(body);
                    Err(ApiError::ApiResponse { status: status.as_u16(), message })?;
                }
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
                            Ok(None) => {},
                            Err(e) => {
                                warn!("SSE parse error: {}", e);
                                Err(e)?;
                            }
                        }
                    }
                }
            }
        };

        Box::pin(stream)
    }

    pub async fn messages(&self, request: MessagesRequest) -> Result<MessagesResponse, ApiError> {
        let req = MessagesRequest { stream: false, ..request };

        let resp = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .json(&req)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            if status.as_u16() == 429 {
                return Err(ApiError::RateLimit { retry_after: None });
            }
            if status.as_u16() == 529 {
                return Err(ApiError::Overloaded);
            }
            if status.as_u16() == 401 {
                return Err(ApiError::Auth(body));
            }
            let message = serde_json::from_str::<ApiErrorResponse>(&body)
                .map(|e| e.error.message)
                .unwrap_or(body);
            return Err(ApiError::ApiResponse { status: status.as_u16(), message });
        }

        let response: MessagesResponse = serde_json::from_str(&body)?;
        Ok(response)
    }
}
