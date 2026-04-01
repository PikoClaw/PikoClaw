use piko_types::message::{ContentBlock, Message, Role, ToolResultContent};
use piko_types::tool::ToolResult;

pub struct ConversationContext {
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    pub max_messages: Option<usize>,
}

impl ConversationContext {
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            messages: Vec::new(),
            max_messages: None,
        }
    }

    pub fn with_system(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.messages.push(Message::user(text));
    }

    pub fn push_assistant_text(&mut self, text: impl Into<String>) {
        self.messages.push(Message::assistant(text));
    }

    pub fn push_assistant_blocks(&mut self, blocks: Vec<ContentBlock>) {
        self.messages.push(Message {
            role: Role::Assistant,
            content: blocks,
        });
    }

    pub fn push_tool_results(&mut self, results: Vec<ToolResult>) {
        let blocks: Vec<ContentBlock> = results
            .into_iter()
            .map(|r| ContentBlock::ToolResult {
                tool_use_id: r.tool_use_id,
                content: ToolResultContent::Text(r.content),
                is_error: if r.is_error { Some(true) } else { None },
            })
            .collect();

        self.messages.push(Message {
            role: Role::User,
            content: blocks,
        });
    }

    pub fn all_messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}
