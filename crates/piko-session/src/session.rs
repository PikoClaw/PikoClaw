use chrono::{DateTime, Utc};
use piko_types::message::Message;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cwd: String,
    pub model: String,
    pub messages: Vec<Message>,
    pub name: Option<String>,
}

impl Session {
    pub fn new(cwd: impl Into<String>, model: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            cwd: cwd.into(),
            model: model.into(),
            messages: Vec::new(),
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cwd: String,
    pub model: String,
    pub name: Option<String>,
    pub message_count: usize,
}

impl From<&Session> for SessionInfo {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.clone(),
            created_at: s.created_at,
            updated_at: s.updated_at,
            cwd: s.cwd.clone(),
            model: s.model.clone(),
            name: s.name.clone(),
            message_count: s.messages.len(),
        }
    }
}
