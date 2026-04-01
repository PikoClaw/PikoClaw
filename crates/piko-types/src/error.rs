use thiserror::Error;

#[derive(Debug, Error)]
pub enum PikoError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Session error: {0}")]
    Session(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}
