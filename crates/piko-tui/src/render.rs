use crate::app::{App, AppState, MessageRole};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

// Claude Code color palette (dark theme)
const COLOR_CLAUDE_ORANGE: Color = Color::Rgb(215, 119, 87);
const COLOR_USER_BG: Color = Color::Rgb(55, 55, 55);
const COLOR_PERMISSION_BLUE: Color = Color::Rgb(87, 105, 247);
const COLOR_SUCCESS: Color = Color::Rgb(44, 122, 57);
const COLOR_ERROR: Color = Color::Rgb(171, 43, 63);
const COLOR_WARNING: Color = Color::Rgb(150, 108, 30);
const COLOR_SUBTLE: Color = Color::Rgb(102, 102, 102);
const COLOR_DIM: Color = Color::Rgb(80, 80, 80);
const COLOR_TEXT: Color = Color::Rgb(220, 220, 220);
const COLOR_INACTIVE: Color = Color::Rgb(120, 120, 120);

// Spinner frames matching Claude Code's ⣾⣽⣻⢿⡿⣟⣯⣷
const SPINNER_FRAMES: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: chat | status bar | input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

    // ── Chat pane ────────────────────────────────────────────────────────────
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .flat_map(|msg| build_message_lines(msg.role.clone(), &msg.content))
        .collect();

    let chat_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DIM))
        .title(Span::styled(
            " PikoClaw ",
            Style::default()
                .fg(COLOR_CLAUDE_ORANGE)
                .add_modifier(Modifier::BOLD),
        ));

    let list = List::new(items)
        .block(chat_block)
        .highlight_style(Style::default().bg(COLOR_USER_BG));
    frame.render_widget(list, chunks[0]);

    // ── Status bar ───────────────────────────────────────────────────────────
    render_status_bar(frame, app, chunks[1]);

    // ── Input / dialog area ──────────────────────────────────────────────────
    if app.state == AppState::AskingPermission {
        render_permission_dialog(frame, app, chunks[2]);
    } else if app.state == AppState::AskingQuestion {
        render_question_dialog(frame, app, chunks[2]);
    } else {
        render_input_bar(frame, app, chunks[2]);
    }
}

// ── Message rendering ────────────────────────────────────────────────────────

fn build_message_lines<'a>(role: MessageRole, content: &'a str) -> Vec<ListItem<'a>> {
    match role {
        MessageRole::User => {
            // User messages: right-aligned label + indented content
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(Span::styled(
                "  You",
                Style::default()
                    .fg(COLOR_TEXT)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in content.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), Style::default().fg(COLOR_TEXT)),
                ]));
            }
            lines.push(Line::from(""));
            vec![ListItem::new(Text::from(lines))]
        }
        MessageRole::Assistant => {
            // Assistant messages: Claude orange label + content
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(Span::styled(
                "  Claude",
                Style::default()
                    .fg(COLOR_CLAUDE_ORANGE)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in content.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), Style::default().fg(COLOR_TEXT)),
                ]));
            }
            lines.push(Line::from(""));
            vec![ListItem::new(Text::from(lines))]
        }
        MessageRole::System => {
            // Tool calls / system messages: dim with icon
            let (icon, color) = classify_system_message(content);
            let lines: Vec<Line> = content
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        Line::from(vec![
                            Span::styled(
                                format!("  {} ", icon),
                                Style::default().fg(color),
                            ),
                            Span::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(color)
                                    .add_modifier(Modifier::DIM),
                            ),
                        ])
                    } else {
                        Line::from(Span::styled(
                            format!("    {}", line),
                            Style::default().fg(COLOR_SUBTLE).add_modifier(Modifier::DIM),
                        ))
                    }
                })
                .collect();
            vec![ListItem::new(Text::from(lines))]
        }
    }
}

fn classify_system_message(content: &str) -> (&'static str, Color) {
    if content.starts_with('[') && content.contains("] running") {
        ("◆", COLOR_PERMISSION_BLUE)
    } else if content.contains("] error") || content.starts_with("Error:") {
        ("✗", COLOR_ERROR)
    } else if content.starts_with("[permission]") {
        ("◈", COLOR_WARNING)
    } else if content.starts_with("[compact]") {
        ("◉", COLOR_SUBTLE)
    } else if content.starts_with("Q:") {
        ("?", COLOR_PERMISSION_BLUE)
    } else if content.starts_with("Commands:") || content.starts_with("Current model") || content.starts_with("Model set") {
        ("›", COLOR_INACTIVE)
    } else {
        ("·", COLOR_SUBTLE)
    }
}

// ── Status bar ───────────────────────────────────────────────────────────────

fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (spinner, state_text, state_color) = match app.state {
        AppState::WaitingForAgent => {
            let frame_idx = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_millis()
                / 50) as usize
                % SPINNER_FRAMES.len();
            (
                SPINNER_FRAMES[frame_idx],
                " Thinking…",
                COLOR_CLAUDE_ORANGE,
            )
        }
        AppState::Running => ("", " Ready", COLOR_SUCCESS),
        AppState::AskingPermission => ("◈", " Permission required", COLOR_WARNING),
        AppState::AskingQuestion => ("?", " Awaiting answer", COLOR_PERMISSION_BLUE),
        AppState::Exiting => ("", " Exiting…", COLOR_SUBTLE),
    };

    let token_info = if app.total_input_tokens > 0 || app.total_output_tokens > 0 {
        let cache_pct = if app.total_input_tokens > 0 {
            (app.total_cache_read_tokens as f32 / app.total_input_tokens as f32 * 100.0) as u32
        } else {
            0
        };
        if cache_pct > 0 {
            format!(
                " ↑{} ↓{} ⚡{}%",
                fmt_tokens(app.total_input_tokens),
                fmt_tokens(app.total_output_tokens),
                cache_pct
            )
        } else {
            format!(
                " ↑{} ↓{}",
                fmt_tokens(app.total_input_tokens),
                fmt_tokens(app.total_output_tokens)
            )
        }
    } else {
        String::new()
    };

    let left = Line::from(vec![
        Span::styled(
            format!(" {} ", spinner),
            Style::default()
                .fg(state_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            state_text,
            Style::default().fg(state_color),
        ),
        Span::styled(
            token_info,
            Style::default().fg(COLOR_INACTIVE),
        ),
    ]);

    let right_text = format!(" pikoclaw ");
    let right = Line::from(Span::styled(
        right_text,
        Style::default()
            .fg(COLOR_CLAUDE_ORANGE)
            .add_modifier(Modifier::BOLD),
    ));

    // Split status bar into left (state) and right (branding)
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(12)])
        .split(area);

    let status_bg = Block::default().style(Style::default().bg(Color::Rgb(28, 28, 28)));
    frame.render_widget(status_bg, area);

    frame.render_widget(
        Paragraph::new(left).style(Style::default().bg(Color::Rgb(28, 28, 28))),
        status_chunks[0],
    );
    frame.render_widget(
        Paragraph::new(right)
            .alignment(Alignment::Right)
            .style(Style::default().bg(Color::Rgb(28, 28, 28))),
        status_chunks[1],
    );
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

// ── Input bar ────────────────────────────────────────────────────────────────

fn render_input_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let cursor_str = if app.state == AppState::Running { "█" } else { "" };
    let display = format!(
        "{}{}{}",
        &app.input[..app.cursor_pos],
        cursor_str,
        &app.input[app.cursor_pos..]
    );

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DIM))
        .title(Span::styled(
            " Message ",
            Style::default().fg(COLOR_INACTIVE),
        ));

    let input_widget = Paragraph::new(display)
        .block(input_block)
        .style(Style::default().fg(COLOR_TEXT))
        .wrap(Wrap { trim: false });

    frame.render_widget(input_widget, area);
}

// ── Permission dialog ────────────────────────────────────────────────────────

fn render_permission_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if let Some(ref prompt) = app.pending_permission {
        let desc = &prompt.request.description;
        let truncated = &desc[..desc.len().min(200)];

        let content = Text::from(vec![
            Line::from(vec![
                Span::styled("Tool:  ", Style::default().fg(COLOR_INACTIVE)),
                Span::styled(
                    prompt.request.tool_name.clone(),
                    Style::default()
                        .fg(COLOR_PERMISSION_BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Action: ", Style::default().fg(COLOR_INACTIVE)),
                Span::styled(truncated.to_string(), Style::default().fg(COLOR_TEXT)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  (y)", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
                Span::styled(" allow  ", Style::default().fg(COLOR_INACTIVE)),
                Span::styled("(a)", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
                Span::styled(" always  ", Style::default().fg(COLOR_INACTIVE)),
                Span::styled("(n)", Style::default().fg(COLOR_ERROR).add_modifier(Modifier::BOLD)),
                Span::styled(" deny  ", Style::default().fg(COLOR_INACTIVE)),
                Span::styled("(d)", Style::default().fg(COLOR_ERROR).add_modifier(Modifier::BOLD)),
                Span::styled(" deny-always", Style::default().fg(COLOR_INACTIVE)),
            ]),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_PERMISSION_BLUE))
            .title(Span::styled(
                " ◈ Permission Required ",
                Style::default()
                    .fg(COLOR_PERMISSION_BLUE)
                    .add_modifier(Modifier::BOLD),
            ));

        frame.render_widget(Paragraph::new(content).block(block).wrap(Wrap { trim: false }), area);
    } else {
        render_input_bar(frame, app, area);
    }
}

// ── Question dialog ──────────────────────────────────────────────────────────

fn render_question_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if let Some(ref prompt) = app.pending_question {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            prompt.question.clone(),
            Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        for (i, opt) in prompt.options.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  ({}) ", i + 1),
                    Style::default()
                        .fg(COLOR_PERMISSION_BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(opt.clone(), Style::default().fg(COLOR_TEXT)),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_PERMISSION_BLUE))
            .title(Span::styled(
                " ? Question ",
                Style::default()
                    .fg(COLOR_PERMISSION_BLUE)
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
