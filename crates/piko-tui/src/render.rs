use crate::app::{App, AppState, MessageRole};
use crate::highlight::{highlight_code, parse_segments, Segment};
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

// ── Spinner frames: same as Claude Code (darwin path) ────────────────────────
// Forward + reversed = smooth back-and-forth animation
const SPINNER_CYCLE: &[&str] = &["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];

// ── Figures (matching constants/figures.ts) ───────────────────────────────────
const BLACK_CIRCLE: &str = "⏺"; // macOS variant (⏺ U+23FA)
const POINTER: &str = "❯"; // figures.pointer

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let t = app.theme;

    // Paint the full-frame background so terminal default doesn't bleed through.
    frame.render_widget(Block::default().style(Style::default().bg(t.bg)), area);

    // 3-row vertical layout: messages | status | input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

    if app.show_header {
        render_header(frame, chunks[0], app);
    } else {
        render_messages(frame, app, chunks[0], t);
    }
    render_status_bar(frame, app, chunks[1], t);

    match app.state {
        AppState::AskingPermission => render_permission_dialog(frame, app, chunks[2], t),
        AppState::AskingQuestion => render_question_dialog(frame, app, chunks[2], t),
        _ => render_input_bar(frame, app, chunks[2], t),
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

fn render_messages(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .flat_map(|msg| message_to_list_items(&msg.role, &msg.content, t))
        .collect();

    let list = List::new(items).style(Style::default().bg(t.bg));
    frame.render_widget(list, area);
}

fn message_to_list_items(role: &MessageRole, content: &str, t: &Theme) -> Vec<ListItem<'static>> {
    match role {
        MessageRole::User => {
            let mut lines: Vec<Line> = Vec::new();
            for line in content.lines() {
                lines.push(Line::from(Span::styled(
                    format!(" {} ", line),
                    Style::default().fg(t.text).bg(t.user_msg_bg),
                )));
            }
            lines.push(Line::from(""));
            vec![ListItem::new(Text::from(lines))]
        }

        MessageRole::Assistant => {
            let mut lines: Vec<Line> = Vec::new();
            let mut first_line = true;

            for segment in parse_segments(content) {
                match segment {
                    Segment::Text(text) => {
                        for raw_line in text.lines() {
                            if first_line {
                                // First line of the whole message gets the ⏺ bullet
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        format!("{} ", BLACK_CIRCLE),
                                        Style::default().fg(t.claude),
                                    ),
                                    Span::styled(raw_line.to_owned(), Style::default().fg(t.text)),
                                ]));
                                first_line = false;
                            } else {
                                lines.push(Line::from(vec![
                                    Span::raw("  "),
                                    Span::styled(raw_line.to_owned(), Style::default().fg(t.text)),
                                ]));
                            }
                        }
                    }
                    Segment::Code { lang, body } => {
                        // ── language label line ──────────────────────────
                        let label = if lang.is_empty() { "code" } else { lang };
                        let label_line = if first_line {
                            first_line = false;
                            Line::from(vec![
                                Span::styled(
                                    format!("{} ", BLACK_CIRCLE),
                                    Style::default().fg(t.claude),
                                ),
                                Span::styled(
                                    label.to_owned(),
                                    Style::default().fg(t.subtle).add_modifier(Modifier::ITALIC),
                                ),
                            ])
                        } else {
                            Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    label.to_owned(),
                                    Style::default().fg(t.subtle).add_modifier(Modifier::ITALIC),
                                ),
                            ])
                        };
                        lines.push(label_line);

                        // ── highlighted code lines ───────────────────────
                        let hl_lines = highlight_code(lang, body, t, "  ");
                        lines.extend(hl_lines);
                    }
                }
            }

            lines.push(Line::from(""));
            vec![ListItem::new(Text::from(lines))]
        }

        MessageRole::System => {
            let (icon, color) = system_icon(content, t);
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
                        Style::default().fg(t.subtle).add_modifier(Modifier::DIM),
                    )));
                }
            }
            vec![ListItem::new(Text::from(lines))]
        }
    }
}

fn system_icon(content: &str, t: &Theme) -> (&'static str, Color) {
    if content.contains("] running") {
        ("◆", t.permission)
    } else if content.contains("] error") || content.starts_with("Error:") {
        ("✗", t.error)
    } else if content.starts_with("[permission]") {
        ("◈", t.warning)
    } else if content.starts_with("[compact]") {
        ("◉", t.subtle)
    } else if content.starts_with("Q:") || content.starts_with("Commands:") {
        ("›", t.inactive)
    } else {
        ("·", t.subtle)
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let (spinner, state_color) = current_spinner(app, t);

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
        Span::styled(token_part, Style::default().fg(t.subtle)),
    ];

    let right_spans = vec![Span::styled(
        format!(" pikoclaw [{}] ", t.name),
        Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
    )];

    let right_width = (t.name.len() as u16) + 13; // " pikoclaw [] " + name
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(right_width)])
        .split(area);

    let bg = Block::default().style(Style::default().bg(t.status_bg));
    frame.render_widget(bg, area);

    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).style(Style::default().bg(t.status_bg)),
        status_chunks[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(right_spans))
            .alignment(Alignment::Right)
            .style(Style::default().bg(t.status_bg)),
        status_chunks[1],
    );
}

fn current_spinner(app: &App, t: &Theme) -> (String, Color) {
    match app.state {
        AppState::WaitingForAgent => {
            let frame = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                / 80) as usize
                % SPINNER_CYCLE.len();
            (SPINNER_CYCLE[frame].to_string(), t.claude)
        }
        AppState::Running => (POINTER.to_string(), t.prompt_border),
        AppState::AskingPermission => ("◈".to_string(), t.permission),
        AppState::AskingQuestion => ("?".to_string(), t.permission),
        AppState::Exiting => ("·".to_string(), t.subtle),
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

fn render_input_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let is_loading = app.state == AppState::WaitingForAgent;

    let pointer_style = if is_loading {
        Style::default()
            .fg(t.prompt_border)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(t.prompt_border)
    };

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
        Span::styled(input_display, Style::default().fg(t.text)),
    ]);

    let border_color = if is_loading {
        t.subtle
    } else {
        t.prompt_border
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    frame.render_widget(
        Paragraph::new(content)
            .block(block)
            .style(Style::default().bg(t.bg))
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ── Permission dialog ─────────────────────────────────────────────────────────

fn render_permission_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    if let Some(ref prompt) = app.pending_permission {
        let desc = &prompt.request.description;
        let truncated = &desc[..desc.len().min(180)];

        let lines = vec![
            Line::from(vec![
                Span::styled("Tool  ", Style::default().fg(t.subtle)),
                Span::styled(
                    prompt.request.tool_name.clone(),
                    Style::default()
                        .fg(t.permission)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled(truncated.to_string(), Style::default().fg(t.inactive)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(t.permission))
            .title(Line::from(vec![
                Span::styled(" Allow ", Style::default().fg(t.permission)),
                Span::styled(
                    "(y)",
                    Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                ),
                Span::styled("es  ", Style::default().fg(t.subtle)),
                Span::styled(
                    "(n)",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled("o  ", Style::default().fg(t.subtle)),
                Span::styled(
                    "(a)",
                    Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                ),
                Span::styled("lways  ", Style::default().fg(t.subtle)),
                Span::styled(
                    "(d)",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled("eny-always ", Style::default().fg(t.subtle)),
            ]));

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    } else {
        render_input_bar(frame, app, area, t);
    }
}

// ── Question dialog ───────────────────────────────────────────────────────────

fn render_question_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    if let Some(ref prompt) = app.pending_question {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            prompt.question.clone(),
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )));
        for (i, opt) in prompt.options.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", i + 1),
                    Style::default()
                        .fg(t.permission)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(opt.clone(), Style::default().fg(t.text)),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(t.permission))
            .title(Span::styled(
                " ? ",
                Style::default()
                    .fg(t.permission)
                    .add_modifier(Modifier::BOLD),
            ));

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    } else {
        render_input_bar(frame, app, area, t);
    }
}

// ── Welcome header ────────────────────────────────────────────────────────────

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = app.theme;
    let width = area.width;

    let version = env!("CARGO_PKG_VERSION");
    let title = Span::styled(
        format!(" PikoClaw v{} ", version),
        Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
    );

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.claude))
        .title(title)
        .style(Style::default().bg(t.bg));

    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    if width >= 70 {
        let left_width = 38u16;
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(left_width), Constraint::Min(1)])
            .split(inner);
        render_header_left(frame, cols[0], app);
        render_header_right(frame, cols[1], app);
    } else {
        render_header_left(frame, inner, app);
    }
}

fn render_header_left(frame: &mut Frame, area: Rect, app: &App) {
    let t = app.theme;
    // Clawd pixel-art: row 1 = head/eyes, row 2 = body, row 3 = feet
    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Welcome back!",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" ▐", Style::default().fg(t.claude)),
            Span::styled("▛███▜", Style::default().fg(t.claude).bg(t.user_msg_bg)),
            Span::styled("▌", Style::default().fg(t.claude)),
        ]),
        Line::from(vec![
            Span::styled("▝▜", Style::default().fg(t.claude)),
            Span::styled("█████", Style::default().fg(t.claude).bg(t.user_msg_bg)),
            Span::styled("▛▘", Style::default().fg(t.claude)),
        ]),
        Line::from(Span::styled("  ▘▘ ▝▝  ", Style::default().fg(t.claude))),
        Line::from(""),
        Line::from(vec![
            Span::styled(&app.model_name, Style::default().fg(t.inactive)),
            Span::styled(" · Claude API", Style::default().fg(t.inactive)),
        ]),
        Line::from(Span::styled(&app.cwd, Style::default().fg(t.inactive))),
    ];

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(t.bg))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_header_right(frame: &mut Frame, area: Rect, app: &App) {
    let t = app.theme;

    // Vertical divider on left edge
    let divider_area = Rect {
        x: area.x,
        y: area.y,
        width: 1,
        height: area.height,
    };
    let content_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    let div_lines: Vec<Line> = (0..divider_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(t.subtle))))
        .collect();
    frame.render_widget(
        Paragraph::new(Text::from(div_lines)).style(Style::default().bg(t.bg)),
        divider_area,
    );

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Tips for getting started",
            Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Run /help to see available commands",
            Style::default().fg(t.text),
        )),
        Line::from(Span::styled(
            "Use /theme [name] to change the color theme",
            Style::default().fg(t.text),
        )),
        Line::from(Span::styled(
            "Use /model <name> to switch models",
            Style::default().fg(t.text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Recent activity",
            Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "No recent activity",
            Style::default().fg(t.inactive),
        )),
    ];

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(t.bg))
            .wrap(Wrap { trim: false }),
        content_area,
    );
}
