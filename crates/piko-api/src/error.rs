use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Rate limited: retry after {retry_after:?}s")]
    RateLimit { retry_after: Option<u64> },
    #[error("API returned error {status}: {message}")]
    ApiResponse { status: u16, message: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SSE parse error: {0}")]
    Sse(String),
    #[error("Overloaded: the API is temporarily unavailable")]
    Overloaded,
}
