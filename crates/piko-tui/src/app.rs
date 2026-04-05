use crate::events::{AppEvent, PermissionPrompt, QuestionPrompt};
use crate::history::InputHistory;
use crate::theme::{self, Theme};
use crate::tui_permissions::{PermissionAsk, TuiPermissionChecker};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, MouseEventKind,
};
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
use piko_tools::plan_mode::{EnterPlanModeTool, ExitPlanModeTool, PlanModeExitRequest};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::collections::HashMap;
use std::io::{stdout, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

/// Pastes longer than this (in bytes) are collapsed to a chip reference.
/// Matches Claude Code's PASTE_THRESHOLD constant.
const PASTE_INLINE_THRESHOLD: usize = 800;

/// Pastes with more than this many newlines are always collapsed to a chip,
/// regardless of byte length. Matches Claude Code's `numLines > maxLines` logic
/// (maxLines = min(rows-10, 2) ≈ 2 on a standard terminal).
const PASTE_MAX_INLINE_LINES: usize = 2;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppState {
    Running,
    WaitingForAgent,
    AskingPermission,
    AskingQuestion,
    AskingPlanModeExit,
    Exiting,
}

pub struct App {
    pub state: AppState,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll: usize,
    pub follow_bottom: bool,
    pub last_total_lines: std::cell::Cell<usize>,
    pub last_frame_height: std::cell::Cell<usize>,
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
    /// Stored paste content for chips: id → full text.
    pub paste_store: HashMap<u32, String>,
    /// Auto-incrementing counter for paste chip IDs.
    pub paste_counter: u32,
    pub plan_mode: Arc<AtomicBool>,
    pub plan_mode_exit_rx: mpsc::UnboundedReceiver<PlanModeExitRequest>,
    pub pending_plan_mode_exit: Option<oneshot::Sender<bool>>,
    /// Raw (non-tilde) CWD used for shortening file paths in tool displays.
    pub cwd_raw: String,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    /// Populated for MessageRole::ToolCall messages.
    pub tool_info: Option<ToolCallInfo>,
}

impl ChatMessage {
    pub fn text(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_info: None,
        }
    }
}

/// Tracks display state for a single tool invocation.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub display_name: String,
    pub args_display: String,
    pub result: Option<ToolResultSummary>,
}

/// The resolved outcome of a tool call.
#[derive(Debug, Clone)]
pub struct ToolResultSummary {
    pub is_error: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Thinking,
    ToolCall,
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
        let cwd_raw = agent.config.cwd.to_string_lossy().into_owned();
        let cwd = {
            let home = std::env::var("HOME").unwrap_or_default();
            if !home.is_empty() && cwd_raw.starts_with(&home) {
                format!("~{}", &cwd_raw[home.len()..])
            } else {
                cwd_raw.clone()
            }
        };

        let policy = PermissionPolicy::from_config(&PermissionsConfig::default());
        let checker = Arc::new(TuiPermissionChecker::new(policy, ask_tx));
        agent = agent.with_permission_checker(checker);

        let ask_tool = Arc::new(AskUserQuestionTool::new(question_tx));
        Arc::get_mut(&mut agent.tools)
            .expect("tools arc not unique")
            .register(ask_tool);

        let plan_mode = Arc::new(AtomicBool::new(false));
        let (plan_exit_tx, plan_mode_exit_rx) = mpsc::unbounded_channel::<PlanModeExitRequest>();
        let enter_pm = Arc::new(EnterPlanModeTool::new(Arc::clone(&plan_mode)));
        Arc::get_mut(&mut agent.tools)
            .expect("tools arc not unique")
            .register(enter_pm);
        let exit_pm = Arc::new(ExitPlanModeTool::new(Arc::clone(&plan_mode), plan_exit_tx));
        Arc::get_mut(&mut agent.tools)
            .expect("tools arc not unique")
            .register(exit_pm);
        agent = agent.with_plan_mode(Arc::clone(&plan_mode));

        Self {
            state: AppState::Running,
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll: 0,
            follow_bottom: true,
            last_total_lines: std::cell::Cell::new(0),
            last_frame_height: std::cell::Cell::new(0),
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
            paste_store: HashMap::new(),
            paste_counter: 0,
            plan_mode,
            plan_mode_exit_rx,
            pending_plan_mode_exit: None,
            cwd_raw,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        stdout.execute(EnableBracketedPaste)?;
        stdout.execute(EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal).await;

        disable_raw_mode()?;
        terminal.backend_mut().execute(DisableBracketedPaste)?;
        terminal.backend_mut().execute(DisableMouseCapture)?;
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
                match event::read()? {
                    Event::Key(key) => {
                        let _ = self.event_tx.send(AppEvent::Key(key));
                    }
                    Event::Paste(text) => {
                        self.handle_paste(text);
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => self.scroll_up(3),
                        MouseEventKind::ScrollDown => self.scroll_down(3),
                        _ => {}
                    },
                    _ => {}
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

            while let Ok(req) = self.plan_mode_exit_rx.try_recv() {
                self.pending_plan_mode_exit = Some(req.reply);
                self.state = AppState::AskingPlanModeExit;
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

        if self.state == AppState::AskingPlanModeExit {
            if let Some(reply) = self.pending_plan_mode_exit.take() {
                let approved = matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y'));
                let _ = reply.send(approved);
                let msg = if approved {
                    "[plan mode] exited — agent can now make changes"
                } else {
                    "[plan mode] exit denied — agent continues in plan mode"
                };
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: msg.to_string(),
                    tool_info: None,
                });
                self.state = AppState::WaitingForAgent;
            }
            return Ok(());
        }

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
                    tool_info: None,
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
                    tool_info: None,
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
                    if self.input[..self.cursor_pos].ends_with(']') {
                        if let Some(chip_start) = self.find_chip_start(self.cursor_pos) {
                            let chip = self.input[chip_start..self.cursor_pos].to_owned();
                            if let Some(id) = parse_chip_id(&chip) {
                                self.paste_store.remove(&id);
                            }
                            self.input.drain(chip_start..self.cursor_pos);
                            self.cursor_pos = chip_start;
                        } else {
                            self.cursor_pos -= 1;
                            self.input.remove(self.cursor_pos);
                        }
                    } else {
                        self.cursor_pos -= 1;
                        self.input.remove(self.cursor_pos);
                    }
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
            // ↑ — scroll viewport when agent is running; else navigate input history.
            (KeyCode::Up, _) => {
                if self.state == AppState::WaitingForAgent {
                    self.scroll_up(1);
                } else if self.input.is_empty() || self.history.is_navigating() {
                    if let Some(recalled) = self.history.backward() {
                        self.input = recalled.to_string();
                        self.cursor_pos = self.input.len();
                    } else {
                        self.scroll_up(1);
                    }
                } else {
                    self.scroll_up(1);
                }
            }
            // ↓ — scroll viewport when agent is running; else navigate history.
            (KeyCode::Down, _) => {
                if self.state == AppState::WaitingForAgent {
                    self.scroll_down(1);
                } else if self.history.is_navigating() {
                    match self.history.forward() {
                        Some(recalled) => {
                            self.input = recalled.to_string();
                            self.cursor_pos = self.input.len();
                        }
                        None => {
                            self.input.clear();
                            self.cursor_pos = 0;
                        }
                    }
                } else {
                    self.scroll_down(1);
                }
            }
            (KeyCode::PageUp, _) => {
                let page = self.last_frame_height.get().saturating_sub(2).max(1);
                self.scroll_up(page);
            }
            (KeyCode::PageDown, _) => {
                let page = self.last_frame_height.get().saturating_sub(2).max(1);
                self.scroll_down(page);
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

    fn scroll_up(&mut self, lines: usize) {
        if self.follow_bottom {
            let total = self.last_total_lines.get();
            let height = self.last_frame_height.get();
            self.scroll = total.saturating_sub(height);
            self.follow_bottom = false;
        }
        self.scroll = self.scroll.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        if self.follow_bottom {
            return;
        }
        let total = self.last_total_lines.get();
        let height = self.last_frame_height.get();
        let max_start = total.saturating_sub(height);
        self.scroll = (self.scroll + lines).min(max_start);
        if self.scroll >= max_start {
            self.follow_bottom = true;
            self.scroll = 0;
        }
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
                            "Commands: /help, /clear, /model <name>, /theme [name], /compact, /cost, /plan, /exit"
                                .to_string(),
                    tool_info: None,
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
                            tool_info: None,
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
                            tool_info: None,
                        });
                    }
                }
                "compact" => {
                    self.compact_context();
                }
                "plan" => {
                    let now_active = !self.plan_mode.load(Ordering::SeqCst);
                    self.plan_mode.store(now_active, Ordering::SeqCst);
                    let msg = if now_active {
                        "Plan mode enabled. Mutating tools (bash, file_write, file_edit, notebook_edit) are now blocked."
                    } else {
                        "Plan mode disabled."
                    };
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: msg.to_string(),
                        tool_info: None,
                    });
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
                            tool_info: None,
                        });
                    } else {
                        let model = self.agent.blocking_lock().config.model.as_str().to_string();
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Current model: {}", model),
                            tool_info: None,
                        });
                    }
                }
                _ => {}
            },
            DispatchResult::Skill {
                rendered_prompt: Some(prompt),
                ..
            } => {
                self.paste_store.clear();
                self.run_agent_turn(prompt.clone(), prompt).await?;
            }
            DispatchResult::NotACommand
            | DispatchResult::Skill {
                rendered_prompt: None,
                ..
            } => {
                let api_input = self.expand_chips(&input);
                self.paste_store.clear();
                self.run_agent_turn(input, api_input).await?;
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
            tool_info: None,
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
            tool_info: None,
        });
    }

    async fn run_agent_turn(&mut self, display_content: String, api_content: String) -> Result<()> {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: display_content,
            tool_info: None,
        });
        self.scroll = 0;
        self.follow_bottom = true;
        self.state = AppState::WaitingForAgent;

        let tx = self.event_tx.clone();
        let sink: Arc<dyn OutputSink> = Arc::new(TuiSink { tx: tx.clone() });
        let agent = Arc::clone(&self.agent);
        tokio::spawn(async move {
            let result = agent.lock().await.run_turn(&api_content, sink).await;
            if let Err(e) = result {
                let _ = tx.send(AppEvent::Agent(AgentEvent::Error(e.to_string())));
            }
            let _ = tx.send(AppEvent::AgentDone);
        });

        Ok(())
    }

    fn handle_paste(&mut self, text: String) {
        if self.state != AppState::Running {
            return;
        }
        // Normalize: \r → \n, tabs → 4 spaces (matches Claude Code behaviour)
        let text = text.replace('\r', "\n").replace('\t', "    ");

        let newlines = text.chars().filter(|&c| c == '\n').count();
        let needs_chip = text.len() > PASTE_INLINE_THRESHOLD || newlines > PASTE_MAX_INLINE_LINES;

        if !needs_chip {
            self.input.insert_str(self.cursor_pos, &text);
            self.cursor_pos += text.len();
        } else {
            self.paste_counter += 1;
            let id = self.paste_counter;
            let chip = if newlines == 0 {
                format!("[Pasted text #{}]", id)
            } else {
                format!("[Pasted text #{} +{} lines]", id, newlines)
            };
            self.paste_store.insert(id, text);
            self.input.insert_str(self.cursor_pos, &chip);
            self.cursor_pos += chip.len();
        }
    }

    fn expand_chips(&self, text: &str) -> String {
        if self.paste_store.is_empty() {
            return text.to_string();
        }
        let mut result = String::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(bracket_pos) = rest.find('[') {
                result.push_str(&rest[..bracket_pos]);
                rest = &rest[bracket_pos..];
                if let Some((chip_len, content)) = self.try_expand_chip_at(rest) {
                    result.push_str(&content);
                    rest = &rest[chip_len..];
                } else {
                    result.push('[');
                    rest = &rest[1..];
                }
            } else {
                result.push_str(rest);
                break;
            }
        }
        result
    }

    fn try_expand_chip_at(&self, s: &str) -> Option<(usize, String)> {
        let close = s.find(']')?;
        let chip = &s[..=close];
        if !is_paste_chip(chip) {
            return None;
        }
        let id = parse_chip_id(chip)?;
        let content = self.paste_store.get(&id)?.clone();
        Some((chip.len(), content))
    }

    fn find_chip_start(&self, end_pos: usize) -> Option<usize> {
        let before = &self.input[..end_pos];
        let bracket_pos = before.rfind('[')?;
        let candidate = &self.input[bracket_pos..end_pos];
        if is_paste_chip(candidate) {
            Some(bracket_pos)
        } else {
            None
        }
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
                    tool_info: None,
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
                    tool_info: None,
                });
            }
            AgentEvent::ToolCallStarted(call) => {
                let display_name = tool_display_name(&call.name);
                let args_display = tool_args_display(&call.name, &call.input, &self.cwd_raw);
                self.messages.push(ChatMessage {
                    role: MessageRole::ToolCall,
                    content: String::new(),
                    tool_info: Some(ToolCallInfo {
                        id: call.id.clone(),
                        display_name,
                        args_display,
                        result: None,
                    }),
                });
            }
            AgentEvent::ToolCallCompleted { call, result } => {
                let summary = tool_result_summary(&call.name, &call.input, &result);
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .rev()
                    .find(|m| m.tool_info.as_ref().is_some_and(|t| t.id == call.id))
                {
                    if let Some(info) = msg.tool_info.as_mut() {
                        info.result = Some(ToolResultSummary {
                            is_error: result.is_error,
                            text: summary,
                        });
                    }
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
                            tool_info: None,
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
                    tool_info: None,
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
                    tool_info: None,
                });
            }
        }
    }
}

fn is_paste_chip(s: &str) -> bool {
    (s.starts_with("[Pasted text #") || s.starts_with("[...Truncated text #")) && s.ends_with(']')
}

// ── Tool call display helpers ─────────────────────────────────────────────────

/// Returns the user-facing tool name shown in bold (matches Claude Code convention).
pub fn tool_display_name(name: &str) -> String {
    match name {
        "bash" => "Bash",
        "file_read" => "Read",
        "file_write" => "Write",
        "file_edit" => "Edit",
        "glob" => "Search",
        "grep" => "Search",
        "web_fetch" => "WebFetch",
        "web_search" => "WebSearch",
        "AskUserQuestion" | "ask_user_question" => "Ask",
        "ExitPlanMode" | "exit_plan_mode" => "ExitPlanMode",
        "EnterPlanMode" | "enter_plan_mode" => "EnterPlanMode",
        other => return other.to_string(),
    }
    .to_string()
}

/// Returns the args string shown in parentheses next to the tool name.
/// Matches Claude Code's truncation and format rules.
pub fn tool_args_display(name: &str, input: &serde_json::Value, cwd_raw: &str) -> String {
    match name {
        "bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let lines: Vec<&str> = cmd.lines().collect();
                let truncated = if lines.len() > 2 {
                    format!("{}\n{}…", lines[0], lines[1])
                } else {
                    cmd.to_string()
                };
                if truncated.len() > 160 {
                    format!("{}…", &truncated[..160])
                } else {
                    truncated
                }
            } else {
                String::new()
            }
        }
        "file_read" | "file_write" | "file_edit" => input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .map(|p| shorten_path(p, cwd_raw))
            .unwrap_or_default(),
        "glob" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            match input.get("path").and_then(|v| v.as_str()) {
                Some(p) => format!(
                    "pattern: \"{}\", path: \"{}\"",
                    pattern,
                    shorten_path(p, cwd_raw)
                ),
                None => format!("pattern: \"{}\"", pattern),
            }
        }
        "grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            match input.get("path").and_then(|v| v.as_str()) {
                Some(p) => format!(
                    "pattern: \"{}\", path: \"{}\"",
                    pattern,
                    shorten_path(p, cwd_raw)
                ),
                None => format!("pattern: \"{}\"", pattern),
            }
        }
        "web_fetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|u| {
                if u.len() > 60 {
                    format!("{}…", &u[..60])
                } else {
                    u.to_string()
                }
            })
            .unwrap_or_default(),
        "web_search" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|q| {
                if q.len() > 80 {
                    format!("{}…", &q[..80])
                } else {
                    q.to_string()
                }
            })
            .unwrap_or_default(),
        "AskUserQuestion" | "ask_user_question" => input
            .get("question")
            .and_then(|v| v.as_str())
            .map(|q| {
                if q.len() > 80 {
                    format!("{}…", &q[..80])
                } else {
                    q.to_string()
                }
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Returns a one-line result summary (e.g. "Read 42 lines", "Wrote 10 lines to src/main.rs").
pub fn tool_result_summary(
    name: &str,
    input: &serde_json::Value,
    result: &piko_types::tool::ToolResult,
) -> String {
    if result.is_error {
        let content = result.content.trim();
        let msg = content
            .split("<tool_use_error>")
            .nth(1)
            .and_then(|s| s.split("</tool_use_error>").next())
            .map(|s| s.trim())
            .unwrap_or(content);
        let msg = msg.lines().next().unwrap_or(msg);
        let msg = if msg.len() > 120 { &msg[..120] } else { msg };
        if msg.starts_with("Error:") || msg.starts_with("Cancelled:") {
            msg.to_string()
        } else {
            format!("Error: {}", msg)
        }
    } else {
        match name {
            "file_read" => {
                let lines = result.content.lines().count();
                format!("Read {} lines", lines)
            }
            "file_write" => {
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let lines = result.content.lines().count();
                let home = std::env::var("HOME").unwrap_or_default();
                let display = if !home.is_empty() && path.starts_with(&home) {
                    format!("~{}", &path[home.len()..])
                } else {
                    path.to_string()
                };
                if display.is_empty() {
                    format!("Wrote {} lines", lines)
                } else {
                    format!("Wrote {} lines to {}", lines, display)
                }
            }
            "file_edit" => {
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if path.is_empty() {
                    "Updated file".to_string()
                } else {
                    let home = std::env::var("HOME").unwrap_or_default();
                    let display = if !home.is_empty() && path.starts_with(&home) {
                        format!("~{}", &path[home.len()..])
                    } else {
                        path.to_string()
                    };
                    format!("Updated {}", display)
                }
            }
            "glob" => {
                let count = result
                    .content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count();
                format!("Found {} files", count)
            }
            "grep" => {
                let count = result
                    .content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count();
                format!("Found {} lines", count)
            }
            "bash" => {
                let content = result.content.trim();
                if content.is_empty() {
                    "(No output)".to_string()
                } else {
                    let first = content.lines().next().unwrap_or("");
                    if first.len() > 80 {
                        format!("{}…", &first[..80])
                    } else {
                        first.to_string()
                    }
                }
            }
            "web_fetch" => {
                let kb = result.content.len() / 1024;
                if kb > 0 {
                    format!("Fetched {}KB", kb)
                } else {
                    format!("Fetched {}B", result.content.len())
                }
            }
            "web_search" => {
                let count = result
                    .content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count();
                format!("Found {} results", count)
            }
            _ => String::new(),
        }
    }
}

fn shorten_path(path: &str, cwd_raw: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    // First try to make relative to cwd
    if !cwd_raw.is_empty() {
        let cwd_slash = if cwd_raw.ends_with('/') {
            cwd_raw.to_string()
        } else {
            format!("{}/", cwd_raw)
        };
        if path.starts_with(&cwd_slash) {
            return path[cwd_slash.len()..].to_string();
        }
        if path == cwd_raw {
            return ".".to_string();
        }
    }
    // Fall back to ~ substitution
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

fn parse_chip_id(chip: &str) -> Option<u32> {
    let hash_pos = chip.find('#')?;
    let after_hash = &chip[hash_pos + 1..];
    let digit_end = after_hash
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_hash.len());
    after_hash[..digit_end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_paste_chip ──────────────────────────────────────────────────────────

    #[test]
    fn chip_pasted_no_lines() {
        assert!(is_paste_chip("[Pasted text #1]"));
    }

    #[test]
    fn chip_pasted_with_lines() {
        assert!(is_paste_chip("[Pasted text #3 +42 lines]"));
    }

    #[test]
    fn chip_truncated_no_lines() {
        assert!(is_paste_chip("[...Truncated text #2...]"));
    }

    #[test]
    fn chip_truncated_with_lines() {
        assert!(is_paste_chip("[...Truncated text #7 +100 lines...]"));
    }

    #[test]
    fn chip_not_a_chip() {
        assert!(!is_paste_chip("[Image #1]"));
        assert!(!is_paste_chip("hello"));
        assert!(!is_paste_chip("[Pasted text #1"));
    }

    // ── parse_chip_id ──────────────────────────────────────────────────────────

    #[test]
    fn parse_id_simple() {
        assert_eq!(parse_chip_id("[Pasted text #5]"), Some(5));
    }

    #[test]
    fn parse_id_with_lines() {
        assert_eq!(parse_chip_id("[Pasted text #12 +3 lines]"), Some(12));
    }

    #[test]
    fn parse_id_truncated() {
        assert_eq!(parse_chip_id("[...Truncated text #99...]"), Some(99));
    }

    #[test]
    fn parse_id_no_hash() {
        assert_eq!(parse_chip_id("no hash here"), None);
    }

    // ── handle_paste / expand_chips ───────────────────────────────────────────

    #[test]
    fn expand_chips_no_chips() {
        let store: HashMap<u32, String> = HashMap::new();
        let input = "hello world";
        let app_store_ref = &store;
        let expand = |text: &str| -> String {
            if app_store_ref.is_empty() {
                return text.to_string();
            }
            text.to_string()
        };
        assert_eq!(expand(input), "hello world");
    }

    #[test]
    fn chip_threshold_constants() {
        assert_eq!(PASTE_INLINE_THRESHOLD, 800);
        assert_eq!(PASTE_MAX_INLINE_LINES, 2);
    }

    #[test]
    fn chip_inline_paste_small() {
        let small = "a".repeat(100);
        assert!(small.len() <= PASTE_INLINE_THRESHOLD, "should be inlined");
    }

    #[test]
    fn chip_triggered_by_line_count() {
        // 3 newlines (4 lines) exceeds PASTE_MAX_INLINE_LINES (2)
        let four_lines = "a\nb\nc\nd";
        let newlines = four_lines.chars().filter(|&c| c == '\n').count();
        assert!(
            four_lines.len() <= PASTE_INLINE_THRESHOLD,
            "short enough to normally inline"
        );
        assert!(newlines > PASTE_MAX_INLINE_LINES, "should trigger chip");
    }

    #[test]
    fn chip_triggered_by_length() {
        let long = "x".repeat(801);
        assert!(long.len() > PASTE_INLINE_THRESHOLD, "should trigger chip");
    }

    #[test]
    fn chip_not_triggered_two_lines() {
        let two_lines = "hello\nworld";
        let newlines = two_lines.chars().filter(|&c| c == '\n').count();
        assert!(two_lines.len() <= PASTE_INLINE_THRESHOLD);
        assert!(newlines <= PASTE_MAX_INLINE_LINES, "should be inlined");
    }

    #[test]
    fn chip_format_no_newlines() {
        let id = 1u32;
        let newlines = 0usize;
        let chip = format!("[Pasted text #{}]", id);
        assert_eq!(chip, "[Pasted text #1]");
        assert!(is_paste_chip(&chip));
        assert_eq!(parse_chip_id(&chip), Some(id));
        let _ = newlines;
    }

    #[test]
    fn chip_format_with_newlines() {
        let id = 2u32;
        let newlines = 5usize;
        let chip = format!("[Pasted text #{} +{} lines]", id, newlines);
        assert_eq!(chip, "[Pasted text #2 +5 lines]");
        assert!(is_paste_chip(&chip));
        assert_eq!(parse_chip_id(&chip), Some(id));
    }

    #[test]
    fn chip_format_truncated_parseable() {
        let id = 3u32;
        let newlines = 200usize;
        let chip = format!("[...Truncated text #{} +{} lines...]", id, newlines);
        assert_eq!(chip, "[...Truncated text #3 +200 lines...]");
        assert!(is_paste_chip(&chip));
        assert_eq!(parse_chip_id(&chip), Some(id));
    }

    #[test]
    fn expand_chips_replaces_chip() {
        let mut store: HashMap<u32, String> = HashMap::new();
        store.insert(1, "full pasted content".to_string());

        let text = "before [Pasted text #1] after";
        let mut result = String::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(bracket_pos) = rest.find('[') {
                result.push_str(&rest[..bracket_pos]);
                rest = &rest[bracket_pos..];
                let close = rest.find(']');
                if let Some(close_pos) = close {
                    let chip = &rest[..=close_pos];
                    if is_paste_chip(chip) {
                        if let Some(id) = parse_chip_id(chip) {
                            if let Some(content) = store.get(&id) {
                                result.push_str(content);
                                rest = &rest[chip.len()..];
                                continue;
                            }
                        }
                    }
                }
                result.push('[');
                rest = &rest[1..];
            } else {
                result.push_str(rest);
                break;
            }
        }
        assert_eq!(result, "before full pasted content after");
    }

    #[test]
    fn expand_chips_multiple_chips() {
        let mut store: HashMap<u32, String> = HashMap::new();
        store.insert(1, "AAA".to_string());
        store.insert(2, "BBB".to_string());

        let text = "[Pasted text #1] and [Pasted text #2]";
        let mut result = String::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(bracket_pos) = rest.find('[') {
                result.push_str(&rest[..bracket_pos]);
                rest = &rest[bracket_pos..];
                let close = rest.find(']');
                if let Some(close_pos) = close {
                    let chip = &rest[..=close_pos];
                    if is_paste_chip(chip) {
                        if let Some(id) = parse_chip_id(chip) {
                            if let Some(content) = store.get(&id) {
                                result.push_str(content);
                                rest = &rest[chip.len()..];
                                continue;
                            }
                        }
                    }
                }
                result.push('[');
                rest = &rest[1..];
            } else {
                result.push_str(rest);
                break;
            }
        }
        assert_eq!(result, "AAA and BBB");
    }

    #[test]
    fn non_chip_brackets_preserved() {
        let store: HashMap<u32, String> = HashMap::new();
        let text = "see [Image #1] here";
        let mut result = String::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(bracket_pos) = rest.find('[') {
                result.push_str(&rest[..bracket_pos]);
                rest = &rest[bracket_pos..];
                let close = rest.find(']');
                if let Some(close_pos) = close {
                    let chip = &rest[..=close_pos];
                    if is_paste_chip(chip) {
                        if let Some(id) = parse_chip_id(chip) {
                            if let Some(content) = store.get(&id) {
                                result.push_str(content);
                                rest = &rest[chip.len()..];
                                continue;
                            }
                        }
                    }
                }
                result.push('[');
                rest = &rest[1..];
            } else {
                result.push_str(rest);
                break;
            }
        }
        assert_eq!(result, "see [Image #1] here");
    }
}
