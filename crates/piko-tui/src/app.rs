use crate::events::AppEvent;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{self, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use piko_agent::agent::Agent;
use piko_agent::output::{AgentEvent, OutputSink};
use piko_skills::dispatcher::{DispatchResult, SkillDispatcher};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppState {
    Running,
    WaitingForAgent,
    Exiting,
}

pub struct App {
    pub state: AppState,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll: usize,
    pub agent: Agent,
    pub dispatcher: SkillDispatcher,
    pub event_tx: mpsc::UnboundedSender<AppEvent>,
    pub event_rx: mpsc::UnboundedReceiver<AppEvent>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

struct TuiSink {
    tx: mpsc::UnboundedSender<AppEvent>,
}

#[async_trait]
impl OutputSink for TuiSink {
    async fn emit(&self, event: AgentEvent) {
        let _ = self.tx.send(AppEvent::Agent(event));
    }
}

impl App {
    pub fn new(agent: Agent, dispatcher: SkillDispatcher) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            state: AppState::Running,
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll: 0,
            agent,
            dispatcher,
            event_tx,
            event_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal).await;

        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        use crate::render::render;

        loop {
            terminal.draw(|frame| render(frame, self))?;

            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    let _ = self.event_tx.send(AppEvent::Key(key));
                }
            }

            while let Ok(event) = self.event_rx.try_recv() {
                match event {
                    AppEvent::Key(key) => self.handle_key(key).await?,
                    AppEvent::Agent(agent_event) => self.handle_agent_event(agent_event),
                    AppEvent::Quit => {
                        self.state = AppState::Exiting;
                    }
                    AppEvent::Tick => {}
                }
            }

            if self.state == AppState::Exiting {
                break;
            }
        }

        Ok(())
    }

    async fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                self.state = AppState::Exiting;
            }
            (KeyCode::Enter, _) if self.state == AppState::Running => {
                let input = std::mem::take(&mut self.input);
                self.cursor_pos = 0;
                if !input.trim().is_empty() {
                    self.submit_input(input).await?;
                }
            }
            (KeyCode::Backspace, _) => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.input.remove(self.cursor_pos);
                }
            }
            (KeyCode::Left, _) => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            (KeyCode::Right, _) => {
                if self.cursor_pos < self.input.len() {
                    self.cursor_pos += 1;
                }
            }
            (KeyCode::Char(c), _) => {
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
        Ok(())
    }

    async fn submit_input(&mut self, input: String) -> Result<()> {
        match self.dispatcher.dispatch(&input) {
            DispatchResult::BuiltIn { name, .. } => match name.as_str() {
                "exit" | "quit" => {
                    self.state = AppState::Exiting;
                }
                "clear" => {
                    self.messages.clear();
                    self.agent.context.messages.clear();
                }
                "help" => {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: "Commands: /help, /clear, /model <name>, /compact, /exit"
                            .to_string(),
                    });
                }
                _ => {}
            },
            DispatchResult::Skill {
                rendered_prompt: Some(prompt),
                ..
            } => {
                self.run_agent_turn(prompt).await?;
            }
            DispatchResult::NotACommand
            | DispatchResult::Skill {
                rendered_prompt: None,
                ..
            } => {
                self.run_agent_turn(input).await?;
            }
        }
        Ok(())
    }

    async fn run_agent_turn(&mut self, input: String) -> Result<()> {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: input.clone(),
        });
        self.state = AppState::WaitingForAgent;

        let tx = self.event_tx.clone();
        let sink: Arc<dyn OutputSink> = Arc::new(TuiSink { tx });
        self.agent.run_turn(&input, sink).await?;
        self.state = AppState::Running;

        Ok(())
    }

    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::TextChunk(text) => {
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.content.push_str(&text);
                        return;
                    }
                }
                self.messages.push(ChatMessage {
                    role: MessageRole::Assistant,
                    content: text,
                });
            }
            AgentEvent::ToolCallStarted(call) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("[{}] running...", call.name),
                });
            }
            AgentEvent::ToolCallCompleted { call, result } => {
                if result.is_error {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!("[{}] error: {}", call.name, result.content),
                    });
                }
            }
            AgentEvent::TurnComplete { .. } => {}
            AgentEvent::Error(msg) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error: {}", msg),
                });
            }
        }
    }
}
