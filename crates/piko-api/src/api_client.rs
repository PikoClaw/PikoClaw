// api_client.rs — Unified ApiClient enum wrapping AnthropicClient and GoogleClient.

use crate::client::AnthropicClient;
use crate::error::ApiError;
use crate::google::GoogleClient;
use crate::request::MessagesRequest;
use crate::response::MessagesResponse;
use crate::stream::EventStream;

pub enum ApiClient {
    Anthropic(AnthropicClient),
    Google(GoogleClient),
}

impl ApiClient {
    pub fn anthropic(
        credential: impl Into<String>,
        base_url: impl Into<String>,
        use_bearer_auth: bool,
        provider: Option<&str>,
    ) -> Result<Self, ApiError> {
        Ok(Self::Anthropic(AnthropicClient::with_options(
            credential,
            base_url,
            use_bearer_auth,
            provider,
        )?))
    }

    pub fn google(api_key: impl Into<String>) -> Self {
        Self::Google(GoogleClient::new(api_key))
    }

    pub fn messages_stream(&self, request: MessagesRequest) -> EventStream {
        match self {
            Self::Anthropic(c) => c.messages_stream(request),
            Self::Google(c) => c.messages_stream(request),
        }
    }

    pub async fn messages(&self, request: MessagesRequest) -> Result<MessagesResponse, ApiError> {
        match self {
            Self::Anthropic(c) => c.messages(request).await,
            Self::Google(c) => c.messages(request).await,
        }
    }
}
