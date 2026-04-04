use crate::events::{AppEvent, PermissionPrompt, QuestionPrompt};
use crate::history::InputHistory;
use crate::theme::{self, Theme};
use crate::tui_permissions::{PermissionAsk, TuiPermissionChecker};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{self, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use piko_agent::agent::Agent;
use piko_agent::output::{AgentEvent, OutputSink};
use piko_config::config::PermissionsConfig;
use piko_permissions::checker::PermissionDecision;
use piko_permissions::policy::PermissionPolicy;
use piko_skills::dispatcher::{DispatchResult, SkillDispatcher};
use piko_tools::ask_user::{AskQuestion, AskUserQuestionTool};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppState {
    Running,
    WaitingForAgent,
    AskingPermission,
    AskingQuestion,
    Exiting,
}

pub struct App {
    pub state: AppState,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll: usize,
    pub agent: Arc<Mutex<Agent>>,
    pub dispatcher: SkillDispatcher,
    pub event_tx: mpsc::UnboundedSender<AppEvent>,
    pub event_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub pending_permission: Option<PermissionPrompt>,
    pub permission_ask_rx: mpsc::UnboundedReceiver<PermissionAsk>,
    pub pending_question: Option<QuestionPrompt>,
    pub question_rx: mpsc::UnboundedReceiver<AskQuestion>,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cache_creation_tokens: u32,
    pub total_cache_read_tokens: u32,
    pub total_cost_usd: f64,
    /// Number of completed turns in this session.
    pub turns: usize,
    /// Optional max session cost in USD. When exceeded, the app exits.
    pub max_budget_usd: Option<f64>,
    /// Active theme (resolved from config or set at runtime via /theme).
    pub theme: &'static Theme,
    /// Show the welcome header until the first message is sent.
    pub show_header: bool,
    /// Model name for header display.
    pub model_name: String,
    /// Working directory for header display.
    pub cwd: String,
    /// Set when a 429 is received; cleared once the instant passes.
    pub rate_limit_until: Option<std::time::Instant>,
    /// Input history for ↑/↓ navigation (Design Spec 03).
    pub history: InputHistory,
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
    Thinking,
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
    pub fn new(
        mut agent: Agent,
        dispatcher: SkillDispatcher,
        theme_name: &str,
        max_budget_usd: Option<f64>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (ask_tx, permission_ask_rx) = mpsc::unbounded_channel::<PermissionAsk>();
        let (question_tx, question_rx) = mpsc::unbounded_channel::<AskQuestion>();

        let model_name = agent.config.model.as_str().to_string();
        let cwd = {
            let raw = agent.config.cwd.to_string_lossy();
            let home = std::env::var("HOME").unwrap_or_default();
            if !home.is_empty() && raw.starts_with(&home) {
                format!("~{}", &raw[home.len()..])
            } else {
                raw.into_owned()
            }
        };

        let policy = PermissionPolicy::from_config(&PermissionsConfig::default());
        let checker = Arc::new(TuiPermissionChecker::new(policy, ask_tx));
        agent = agent.with_permission_checker(checker);

        let ask_tool = Arc::new(AskUserQuestionTool::new(question_tx));
        Arc::get_mut(&mut agent.tools)
            .expect("tools arc not unique")
            .register(ask_tool);

        Self {
            state: AppState::Running,
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll: 0,
            agent: Arc::new(Mutex::new(agent)),
            dispatcher,
            event_tx,
            event_rx,
            pending_permission: None,
            permission_ask_rx,
            pending_question: None,
            question_rx,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            total_cost_usd: 0.0,
            turns: 0,
            max_budget_usd,
            theme: theme::by_name(theme_name),
            show_header: true,
            model_name,
            cwd,
            rate_limit_until: None,
            history: InputHistory::new(),
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

            while let Ok(ask) = self.permission_ask_rx.try_recv() {
                let prompt = PermissionPrompt {
                    request: ask.request,
                    reply: ask.reply,
                };
                self.pending_permission = Some(prompt);
                self.state = AppState::AskingPermission;
            }

            while let Ok(ask) = self.question_rx.try_recv() {
                let prompt = QuestionPrompt {
                    question: ask.question,
                    options: ask.options,
                    reply: ask.reply,
                };
                self.pending_question = Some(prompt);
                self.state = AppState::AskingQuestion;
            }

            while let Ok(event) = self.event_rx.try_recv() {
                match event {
                    AppEvent::Key(key) => self.handle_key(key).await?,
                    AppEvent::Agent(agent_event) => self.handle_agent_event(agent_event),
                    AppEvent::AgentDone => {
                        if self.state == AppState::WaitingForAgent {
                            self.state = AppState::Running;
                        }
                    }
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

        if self.state == AppState::AskingQuestion {
            if let Some(prompt) = self.pending_question.take() {
                let answer = match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let idx = c as usize - '1' as usize;
                        prompt.options.get(idx).cloned().unwrap_or_default()
                    }
                    _ => prompt.options.first().cloned().unwrap_or_default(),
                };
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Q: {} → {}", prompt.question, answer),
                });
                let _ = prompt.reply.send(answer);
                self.state = AppState::WaitingForAgent;
            }
            return Ok(());
        }

        if self.state == AppState::AskingPermission {
            if let Some(prompt) = self.pending_permission.take() {
                let decision = match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => PermissionDecision::Allow,
                    KeyCode::Char('a') | KeyCode::Char('A') => PermissionDecision::AllowAlways,
                    KeyCode::Char('d') | KeyCode::Char('D') => PermissionDecision::DenyAlways,
                    _ => PermissionDecision::Deny,
                };
                let tool = prompt.request.tool_name.clone();
                let decided = format!("{:?}", decision);
                let _ = prompt.reply.send(decision);
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("[permission] {} → {}", tool, decided),
                });
                self.state = AppState::WaitingForAgent;
            }
            return Ok(());
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                self.state = AppState::Exiting;
            }
            (KeyCode::Enter, KeyModifiers::SHIFT) => {
                self.input.insert(self.cursor_pos, '\n');
                self.cursor_pos += 1;
            }
            (KeyCode::Enter, _) if self.state == AppState::Running => {
                let input = std::mem::take(&mut self.input);
                self.cursor_pos = 0;
                self.history.reset();
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
            // ↑ — navigate to previous history entry when input is empty or already browsing.
            // Falls back to chat-pane scrolling when there is no history to recall.
            (KeyCode::Up, _) => {
                if self.input.is_empty() || self.history.is_navigating() {
                    if let Some(recalled) = self.history.backward() {
                        self.input = recalled.to_string();
                        self.cursor_pos = self.input.len();
                    } else {
                        self.scroll = self.scroll.saturating_sub(1);
                    }
                } else {
                    self.scroll = self.scroll.saturating_sub(1);
                }
            }
            // ↓ — navigate forward in history while browsing; scroll chat otherwise.
            (KeyCode::Down, _) => {
                if self.history.is_navigating() {
                    match self.history.forward() {
                        Some(recalled) => {
                            self.input = recalled.to_string();
                            self.cursor_pos = self.input.len();
                        }
                        None => {
                            // Past the newest entry → return to empty live input.
                            self.input.clear();
                            self.cursor_pos = 0;
                        }
                    }
                } else {
                    self.scroll = self.scroll.saturating_add(1);
                }
            }
            (KeyCode::Char(c), _) => {
                // Typing while browsing history exits navigation mode but keeps the
                // recalled text so the user can edit it.
                if self.history.is_navigating() {
                    self.history.reset();
                }
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
        Ok(())
    }

    async fn submit_input(&mut self, input: String) -> Result<()> {
        // Record every non-empty submission in input history before dispatching.
        self.history.push(input.clone());
        self.show_header = false;
        match self.dispatcher.dispatch(&input) {
            DispatchResult::BuiltIn { name, args } => match name.as_str() {
                "exit" | "quit" => {
                    self.state = AppState::Exiting;
                }
                "clear" => {
                    self.messages.clear();
                    self.agent.lock().await.context.messages.clear();
                }
                "help" => {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content:
                            "Commands: /help, /clear, /model <name>, /theme [name], /compact, /exit"
                                .to_string(),
                    });
                }
                "theme" => {
                    if let Some(name) = args.first() {
                        // Set a specific theme by name
                        let t = theme::by_name(name);
                        self.theme = t;
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Theme set to «{}»", t.label),
                        });
                    } else {
                        // No arg → cycle to next theme
                        let t = theme::next(self.theme.name);
                        self.theme = t;
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!(
                                "Theme → «{}»  (available: {})",
                                t.label,
                                theme::ALL_THEMES
                                    .iter()
                                    .map(|t| t.name)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        });
                    }
                }
                "compact" => {
                    self.compact_context();
                }
                "cost" => {
                    self.show_cost_summary();
                }
                "model" => {
                    if let Some(model_name) = args.first() {
                        use piko_types::model::ModelId;
                        let model = ModelId::from_alias(model_name);
                        self.agent.blocking_lock().config.model = model.clone();
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Model set to {}", model.as_str()),
                        });
                    } else {
                        let model = self.agent.blocking_lock().config.model.as_str().to_string();
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Current model: {}", model),
                        });
                    }
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

    fn show_cost_summary(&mut self) {
        let pricing = piko_api::get_pricing(&self.model_name);
        let total = self.total_cost_usd;
        let input_cost = (self.total_input_tokens as f64 / 1_000_000.0) * pricing.input_per_m;
        let output_cost = (self.total_output_tokens as f64 / 1_000_000.0) * pricing.output_per_m;
        let cw_cost =
            (self.total_cache_creation_tokens as f64 / 1_000_000.0) * pricing.cache_write_per_m;
        let cr_cost =
            (self.total_cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_read_per_m;
        let savings_from_cache = cw_cost + cr_cost;

        let content = if total == 0.0 {
            "Session Cost Summary\n────────────────────────────────────\nNo turns made yet."
                .to_string()
        } else {
            format!(
                "Session Cost Summary\n\
                 ────────────────────────────────────\n\
                 Model:          {}\n\
                 Turns:          {}\n\
                 \n\
                 Token Usage:\n\
                 \u{2003} Input:        {}  \u{2192}  {}\n\
                 \u{2003} Output:       {}  \u{2192}  {}\n\
                 \u{2003} Cache write:  {}  \u{2192}  {}\n\
                 \u{2003} Cache read:   {}  \u{2192}  {}\n\
                 \u{2003}                            {}\n\
                 \u{2003} Total:                   {}\n\
                 \n\
                 Savings from cache: {} (compared to no caching)",
                self.model_name,
                self.turns,
                self.total_input_tokens,
                piko_api::format_cost(input_cost),
                self.total_output_tokens,
                piko_api::format_cost(output_cost),
                self.total_cache_creation_tokens,
                piko_api::format_cost(cw_cost),
                self.total_cache_read_tokens,
                piko_api::format_cost(cr_cost),
                "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                piko_api::format_cost(total),
                piko_api::format_cost(savings_from_cache),
            )
        };
        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content,
        });
    }

    fn compact_context(&mut self) {
        let mut agent = self.agent.blocking_lock();
        let summary: String = agent
            .context
            .messages
            .iter()
            .filter_map(|m| {
                use piko_types::message::{ContentBlock, Role};
                let prefix = match m.role {
                    Role::User => "User",
                    Role::Assistant => "Assistant",
                };
                let text: String = m
                    .content
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if text.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", prefix, &text[..text.len().min(200)]))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        agent.context.messages.clear();
        drop(agent);
        self.messages.clear();
        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: format!("[compact] conversation summarized:\n{}", summary),
        });
    }

    async fn run_agent_turn(&mut self, input: String) -> Result<()> {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: input.clone(),
        });
        self.state = AppState::WaitingForAgent;

        let tx = self.event_tx.clone();
        let sink: Arc<dyn OutputSink> = Arc::new(TuiSink { tx: tx.clone() });
        let agent = Arc::clone(&self.agent);
        tokio::spawn(async move {
            let result = agent.lock().await.run_turn(&input, sink).await;
            if let Err(e) = result {
                let _ = tx.send(AppEvent::Agent(AgentEvent::Error(e.to_string())));
            }
            let _ = tx.send(AppEvent::AgentDone);
        });

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
            AgentEvent::ThinkingChunk(text) => {
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Thinking {
                        last.content.push_str(&text);
                        return;
                    }
                }
                self.messages.push(ChatMessage {
                    role: MessageRole::Thinking,
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
            AgentEvent::TurnComplete {
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
            } => {
                let pricing = piko_api::get_pricing(&self.model_name);
                let cost = piko_api::calculate_cost_raw(
                    input_tokens,
                    output_tokens,
                    cache_creation_tokens,
                    cache_read_tokens,
                    &pricing,
                );
                self.total_cost_usd += cost;
                self.total_input_tokens += input_tokens;
                self.total_output_tokens += output_tokens;
                self.total_cache_creation_tokens += cache_creation_tokens;
                self.total_cache_read_tokens += cache_read_tokens;
                self.turns += 1;
                // Budget enforcement check
                if let Some(max) = self.max_budget_usd {
                    if self.total_cost_usd >= max {
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!(
                                "Budget limit reached ({}). Session stopped.",
                                piko_api::format_cost(max)
                            ),
                        });
                        self.state = AppState::Exiting;
                    }
                }
                tracing::debug!("[cost] turn complete: cost_usd={:.4}", cost);
            }
            AgentEvent::Error(msg) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error: {}", msg),
                });
            }
            AgentEvent::RateLimit { retry_after } => {
                let reset_in = retry_after.unwrap_or(60);
                self.rate_limit_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(reset_in));
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!(
                        "Rate limited · resets in {}m {}s",
                        reset_in / 60,
                        reset_in % 60
                    ),
                });
            }
        }
    }
}
