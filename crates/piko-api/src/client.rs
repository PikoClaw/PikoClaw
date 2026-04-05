use crate::error::ApiError;
use crate::request::MessagesRequest;
use crate::response::{ApiErrorResponse, MessagesResponse};
use crate::stream::{parse_sse_line, EventStream};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use tracing::{debug, warn};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

pub struct AnthropicClient {
    http: reqwest::Client,
    /// The credential value (API key or OAuth/Bearer token).
    api_key: String,
    base_url: String,
    /// When `true`, send `Authorization: Bearer <api_key>` instead of `x-api-key`.
    /// Use this for third-party providers such as OpenRouter that accept Bearer tokens
    /// via ANTHROPIC_AUTH_TOKEN.
    use_bearer_auth: bool,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, ApiError> {
        Self::with_options(api_key, DEFAULT_BASE_URL, false)
    }

    pub fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, ApiError> {
        Self::with_options(api_key, base_url, false)
    }

    /// Full constructor used when a custom base URL and/or Bearer auth is required
    /// (e.g. OpenRouter with ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN).
    pub fn with_options(
        credential: impl Into<String>,
        base_url: impl Into<String>,
        use_bearer_auth: bool,
    ) -> Result<Self, ApiError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            http,
            api_key: credential.into(),
            base_url: base_url.into(),
            use_bearer_auth,
        })
    }

    pub fn messages_stream(&self, request: MessagesRequest) -> EventStream {
        let http = self.http.clone();
        let api_key = self.api_key.clone();
        let url = format!("{}/v1/messages", self.base_url);
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
                http.post(&url).header("Authorization", format!("Bearer {api_key}"))
            } else {
                http.post(&url).header("x-api-key", &api_key)
            };
            if let Some(ref betas) = req.betas {
                builder = builder.header("anthropic-beta", betas.join(","));
            }
            let resp = builder.json(&payload).send().await?;

            let status = resp.status();

            if !status.is_success() {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                let body = resp.text().await.unwrap_or_default();
                if status.as_u16() == 429 {
                    Err(ApiError::RateLimit { retry_after })?;
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
        let req = MessagesRequest {
            stream: false,
            ..request
        };

        let mut req_builder = self.http.post(format!("{}/v1/messages", self.base_url));
        req_builder = if self.use_bearer_auth {
            req_builder.header("Authorization", format!("Bearer {}", &self.api_key))
        } else {
            req_builder.header("x-api-key", &self.api_key)
        };
        let resp = req_builder.json(&req).send().await?;

        let status = resp.status();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let body = resp.text().await?;

        if !status.is_success() {
            if status.as_u16() == 429 {
                return Err(ApiError::RateLimit { retry_after });
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
            return Err(ApiError::ApiResponse {
                status: status.as_u16(),
                message,
            });
        }

        let response: MessagesResponse = serde_json::from_str(&body)?;
        Ok(response)
    }
}
