use crate::app::{App, AppState, MessageRole};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

// ── Exact Claude Code dark theme RGB palette ──────────────────────────────────
const CLAUDE_ORANGE: Color = Color::Rgb(215, 119, 87); // claude / brand
const PERMISSION_BLUE: Color = Color::Rgb(177, 185, 249); // permission
const PROMPT_BORDER: Color = Color::Rgb(136, 136, 136); // promptBorder
const TEXT: Color = Color::Rgb(255, 255, 255); // text
const INACTIVE: Color = Color::Rgb(153, 153, 153); // inactive
const SUBTLE: Color = Color::Rgb(80, 80, 80); // subtle
const SUCCESS: Color = Color::Rgb(78, 186, 101); // success
const ERROR_RED: Color = Color::Rgb(255, 107, 128); // error
const WARNING: Color = Color::Rgb(255, 193, 7); // warning
const USER_MSG_BG: Color = Color::Rgb(55, 55, 55); // userMessageBackground
const STATUS_BG: Color = Color::Rgb(20, 20, 20); // status bar background

// ── Spinner frames: same as Claude Code (darwin path) ────────────────────────
// Forward + reversed = smooth back-and-forth animation
// Full cycle: forward then reverse (without duplicating endpoints)
const SPINNER_CYCLE: &[&str] = &["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];

// ── Figures (matching constants/figures.ts) ───────────────────────────────────
const BLACK_CIRCLE: &str = "⏺"; // macOS variant (⏺ U+23FA)
const POINTER: &str = "❯"; // figures.pointer

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 3-row vertical layout: messages | status | input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

    render_messages(frame, app, chunks[0]);
    render_status_bar(frame, app, chunks[1]);

    match app.state {
        AppState::AskingPermission => render_permission_dialog(frame, app, chunks[2]),
        AppState::AskingQuestion => render_question_dialog(frame, app, chunks[2]),
        _ => render_input_bar(frame, app, chunks[2]),
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

fn render_messages(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .flat_map(|msg| message_to_list_items(&msg.role, &msg.content))
        .collect();

    // No border on the chat area — Claude Code uses borderless scrollback
    let list = List::new(items);
    frame.render_widget(list, area);
}

fn message_to_list_items(role: &MessageRole, content: &str) -> Vec<ListItem<'static>> {
    match role {
        MessageRole::User => {
            // User messages: rgb(55,55,55) background block, no prefix label
            // Matches UserPromptMessage.tsx: backgroundColor="userMessageBackground"
            let mut lines: Vec<Line> = Vec::new();
            for line in content.lines() {
                lines.push(Line::from(Span::styled(
                    format!(" {} ", line),
                    Style::default().fg(TEXT).bg(USER_MSG_BG),
                )));
            }
            // Empty line after message (no background) for spacing
            lines.push(Line::from(""));
            vec![ListItem::new(Text::from(lines))]
        }

        MessageRole::Assistant => {
            // Assistant messages: ⏺ dot on first line, then plain text
            // Matches AssistantTextMessage.tsx: BLACK_CIRCLE + Markdown text
            let mut lines: Vec<Line> = Vec::new();
            let content_lines: Vec<&str> = content.lines().collect();
            for (i, line) in content_lines.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{} ", BLACK_CIRCLE),
                            Style::default().fg(CLAUDE_ORANGE),
                        ),
                        Span::styled(line.to_string(), Style::default().fg(TEXT)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "), // 2-char indent matching dot + space width
                        Span::styled(line.to_string(), Style::default().fg(TEXT)),
                    ]));
                }
            }
            lines.push(Line::from(""));
            vec![ListItem::new(Text::from(lines))]
        }

        MessageRole::System => {
            // Tool calls, system info: dimmed with semantic icon
            let (icon, color) = system_icon(content);
            let mut lines: Vec<Line> = Vec::new();
            for (i, line) in content.lines().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{} ", icon),
                            Style::default().fg(color).add_modifier(Modifier::DIM),
                        ),
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(color).add_modifier(Modifier::DIM),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", line),
                        Style::default().fg(SUBTLE).add_modifier(Modifier::DIM),
                    )));
                }
            }
            vec![ListItem::new(Text::from(lines))]
        }
    }
}

fn system_icon(content: &str) -> (&'static str, Color) {
    if content.contains("] running") {
        ("◆", PERMISSION_BLUE)
    } else if content.contains("] error") || content.starts_with("Error:") {
        ("✗", ERROR_RED)
    } else if content.starts_with("[permission]") {
        ("◈", WARNING)
    } else if content.starts_with("[compact]") {
        ("◉", SUBTLE)
    } else if content.starts_with("Q:") || content.starts_with("Commands:") {
        ("›", INACTIVE)
    } else {
        ("·", SUBTLE)
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────
// Matches StatusLine.tsx: dim text, model · cwd · tokens · context%

fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // Spinner + state on left, branding on right
    let (spinner, state_color) = current_spinner(app);

    // Token display — matching StatusLine format
    let token_part = if app.total_input_tokens > 0 || app.total_output_tokens > 0 {
        let cache_pct = if app.total_input_tokens > 0 {
            (app.total_cache_read_tokens as f32 / app.total_input_tokens as f32 * 100.0) as u32
        } else {
            0
        };
        if cache_pct > 0 {
            format!(
                " · ↑{} ↓{} ({}% cached)",
                fmt_tokens(app.total_input_tokens),
                fmt_tokens(app.total_output_tokens),
                cache_pct
            )
        } else {
            format!(
                " · ↑{} ↓{}",
                fmt_tokens(app.total_input_tokens),
                fmt_tokens(app.total_output_tokens)
            )
        }
    } else {
        String::new()
    };

    let left_spans = vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(state_color)),
        Span::styled(token_part, Style::default().fg(SUBTLE)),
    ];

    let right_spans = vec![Span::styled(
        " pikoclaw ",
        Style::default()
            .fg(CLAUDE_ORANGE)
            .add_modifier(Modifier::BOLD),
    )];

    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(12)])
        .split(area);

    // Dark background fill
    let bg = Block::default().style(Style::default().bg(STATUS_BG));
    frame.render_widget(bg, area);

    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).style(Style::default().bg(STATUS_BG)),
        status_chunks[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(right_spans))
            .alignment(Alignment::Right)
            .style(Style::default().bg(STATUS_BG)),
        status_chunks[1],
    );
}

fn current_spinner(app: &App) -> (String, Color) {
    match app.state {
        AppState::WaitingForAgent => {
            // Animate through SPINNER_CYCLE using wall-clock time at ~80ms per frame
            let frame = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                / 80) as usize
                % SPINNER_CYCLE.len();
            (SPINNER_CYCLE[frame].to_string(), CLAUDE_ORANGE)
        }
        AppState::Running => (POINTER.to_string(), PROMPT_BORDER),
        AppState::AskingPermission => ("◈".to_string(), PERMISSION_BLUE),
        AppState::AskingQuestion => ("?".to_string(), PERMISSION_BLUE),
        AppState::Exiting => ("·".to_string(), SUBTLE),
    }
}

fn fmt_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f32 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f32 / 1_000.0)
    } else {
        n.to_string()
    }
}

// ── Input bar ─────────────────────────────────────────────────────────────────
// Matches PromptInput: rounded border (borderStyle="round"), promptBorder color,
// ❯ prefix (figures.pointer), cursor as block █

fn render_input_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let is_loading = app.state == AppState::WaitingForAgent;

    // ❯ prompt char — dimmed while loading (matches PromptChar dimColor={isLoading})
    let pointer_style = if is_loading {
        Style::default()
            .fg(PROMPT_BORDER)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(PROMPT_BORDER)
    };

    // Cursor visible only while running
    let cursor = if app.state == AppState::Running {
        "█"
    } else {
        ""
    };
    let input_display = format!(
        "{}{}{}",
        &app.input[..app.cursor_pos],
        cursor,
        &app.input[app.cursor_pos..]
    );

    let content = Line::from(vec![
        Span::styled(format!("{} ", POINTER), pointer_style),
        Span::styled(input_display, Style::default().fg(TEXT)),
    ]);

    // Round border matching borderStyle="round" in PromptInput.tsx
    // Only top border visible (borderLeft=false, borderRight=false, borderBottom=true in Claude Code)
    // Ratatui doesn't support per-side rounded — use ALL borders with rounded type
    let border_color = if is_loading { SUBTLE } else { PROMPT_BORDER };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    frame.render_widget(
        Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ── Permission dialog ─────────────────────────────────────────────────────────
// Matches PermissionPrompt.tsx: box with divider ▔, permission color border,
// options with key hints

fn render_permission_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if let Some(ref prompt) = app.pending_permission {
        let desc = &prompt.request.description;
        let truncated = &desc[..desc.len().min(180)];

        let lines = vec![
            Line::from(vec![
                Span::styled("Tool  ", Style::default().fg(SUBTLE)),
                Span::styled(
                    prompt.request.tool_name.clone(),
                    Style::default()
                        .fg(PERMISSION_BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled(truncated.to_string(), Style::default().fg(INACTIVE)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(PERMISSION_BLUE))
            .title(Line::from(vec![
                Span::styled(" Allow ", Style::default().fg(PERMISSION_BLUE)),
                Span::styled(
                    "(y)",
                    Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD),
                ),
                Span::styled("es  ", Style::default().fg(SUBTLE)),
                Span::styled(
                    "(n)",
                    Style::default().fg(ERROR_RED).add_modifier(Modifier::BOLD),
                ),
                Span::styled("o  ", Style::default().fg(SUBTLE)),
                Span::styled(
                    "(a)",
                    Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD),
                ),
                Span::styled("lways  ", Style::default().fg(SUBTLE)),
                Span::styled(
                    "(d)",
                    Style::default().fg(ERROR_RED).add_modifier(Modifier::BOLD),
                ),
                Span::styled("eny-always ", Style::default().fg(SUBTLE)),
            ]));

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    } else {
        render_input_bar(frame, app, area);
    }
}

// ── Question dialog ───────────────────────────────────────────────────────────

fn render_question_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if let Some(ref prompt) = app.pending_question {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            prompt.question.clone(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )));
        for (i, opt) in prompt.options.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", i + 1),
                    Style::default()
                        .fg(PERMISSION_BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(opt.clone(), Style::default().fg(TEXT)),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(PERMISSION_BLUE))
            .title(Span::styled(
                " ? ",
                Style::default()
                    .fg(PERMISSION_BLUE)
                    .add_modifier(Modifier::BOLD),
            ));

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    } else {
        render_input_bar(frame, app, area);
    }
}
