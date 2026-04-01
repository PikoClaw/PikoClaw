use crossterm::event::KeyEvent;
use piko_agent::AgentEvent;
use piko_permissions::checker::{PermissionDecision, PermissionRequest};
use tokio::sync::oneshot;

pub struct PermissionPrompt {
    pub request: PermissionRequest,
    pub reply: oneshot::Sender<PermissionDecision>,
}

pub struct QuestionPrompt {
    pub question: String,
    pub options: Vec<String>,
    pub reply: oneshot::Sender<String>,
}

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Agent(AgentEvent),
    Tick,
    Quit,
}
