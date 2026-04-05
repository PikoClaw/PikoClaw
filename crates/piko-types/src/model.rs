use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(pub String);

impl ModelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn claude_sonnet_4_5() -> Self {
        Self("claude-sonnet-4-5".to_string())
    }

    pub fn claude_opus_4_6() -> Self {
        Self("claude-opus-4-6".to_string())
    }

    pub fn claude_haiku_4_5() -> Self {
        Self("claude-haiku-4-5-20251001".to_string())
    }

    pub fn from_alias(alias: &str) -> Self {
        match alias {
            "sonnet" => Self::claude_sonnet_4_5(),
            "opus" => Self::claude_opus_4_6(),
            "haiku" => Self::claude_haiku_4_5(),
            other => Self::new(other),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ModelId {
    fn default() -> Self {
        Self::claude_sonnet_4_5()
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for ModelId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for ModelId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ModelId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
