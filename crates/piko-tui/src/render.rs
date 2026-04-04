use crate::app::{App, AppState, MessageRole};
use crate::highlight::{highlight_code, parse_inline_spans, parse_segments, Segment};
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use std::sync::atomic::Ordering;

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
        AppState::AskingPlanModeExit => render_plan_mode_exit_dialog(frame, chunks[2], t),
        _ => render_input_bar(frame, app, chunks[2], t),
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

fn render_messages(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let all_lines: Vec<Line> = app
        .messages
        .iter()
        .flat_map(|msg| message_to_lines(&msg.role, &msg.content, t))
        .collect();

    let total = all_lines.len();
    let height = area.height as usize;

    // scroll=0 → show bottom (most recent); scroll=N → N lines above bottom.
    let max_scroll = total.saturating_sub(height);
    let scroll = app.scroll.min(max_scroll);

    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(end.min(height));

    let visible: Vec<Line> = all_lines[start..end].to_vec();
    frame.render_widget(
        Paragraph::new(Text::from(visible)).style(Style::default().bg(t.bg)),
        area,
    );
}

/// Render a block-level markdown line (headings, lists, blockquotes, HR) into spans.
/// Falls back to inline parsing for ordinary text.
fn block_line_spans(
    line: &str,
    text_style: Style,
    code_style: Style,
    t: &Theme,
) -> Vec<Span<'static>> {
    // Headings: # / ## / ### … ######
    let hash_count = line.bytes().take_while(|&b| b == b'#').count();
    if hash_count > 0
        && hash_count <= 6
        && line.len() > hash_count
        && line.as_bytes()[hash_count] == b' '
    {
        let rest = &line[hash_count + 1..];
        let style = if hash_count == 1 {
            text_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            text_style.add_modifier(Modifier::BOLD)
        };
        return parse_inline_spans(rest, style, code_style);
    }

    // Unordered list: "- ", "* ", "+ "
    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let mut spans = vec![Span::styled("• ".to_owned(), Style::default().fg(t.subtle))];
            spans.extend(parse_inline_spans(rest, text_style, code_style));
            return spans;
        }
    }

    // Ordered list: "N. "
    if let Some(dot_pos) = line.find(". ") {
        let prefix = &line[..dot_pos];
        if !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_digit()) {
            let rest = &line[dot_pos + 2..];
            let mut spans = vec![Span::styled(
                format!("{}. ", prefix),
                Style::default().fg(t.subtle),
            )];
            spans.extend(parse_inline_spans(rest, text_style, code_style));
            return spans;
        }
    }

    // Blockquote: "> "
    if let Some(rest) = line.strip_prefix("> ") {
        let mut spans = vec![Span::styled(
            "│ ".to_owned(),
            Style::default().fg(t.subtle).add_modifier(Modifier::DIM),
        )];
        spans.extend(parse_inline_spans(
            rest,
            text_style.add_modifier(Modifier::ITALIC),
            code_style,
        ));
        return spans;
    }

    // Horizontal rule
    if matches!(line, "---" | "***" | "___") {
        return vec![Span::styled(
            "─".repeat(40),
            Style::default().fg(t.subtle).add_modifier(Modifier::DIM),
        )];
    }

    parse_inline_spans(line, text_style, code_style)
}

fn message_to_lines(role: &MessageRole, content: &str, t: &Theme) -> Vec<Line<'static>> {
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
            lines
        }

        MessageRole::Assistant => {
            let mut lines: Vec<Line> = Vec::new();
            let mut first_line = true;
            let text_style = Style::default().fg(t.text);
            let code_style = Style::default().fg(t.permission);

            for segment in parse_segments(content) {
                match segment {
                    Segment::Text(text) => {
                        for raw_line in text.lines() {
                            let content_spans =
                                block_line_spans(raw_line, text_style, code_style, t);
                            let line = if first_line {
                                first_line = false;
                                let mut spans = vec![Span::styled(
                                    format!("{} ", BLACK_CIRCLE),
                                    Style::default().fg(t.claude),
                                )];
                                spans.extend(content_spans);
                                Line::from(spans)
                            } else {
                                let mut spans = vec![Span::raw("  ")];
                                spans.extend(content_spans);
                                Line::from(spans)
                            };
                            lines.push(line);
                        }
                    }
                    Segment::Code { lang, body } => {
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
                        lines.extend(highlight_code(lang, body, t, "  "));
                    }
                }
            }

            lines.push(Line::from(""));
            lines
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
            lines
        }

        MessageRole::Thinking => {
            let mut lines: Vec<Line> = Vec::new();
            let mut first = true;
            for line in content.lines() {
                if first {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "▷ ",
                            Style::default()
                                .fg(t.subtle)
                                .add_modifier(Modifier::DIM | Modifier::ITALIC),
                        ),
                        Span::styled(
                            line.to_owned(),
                            Style::default()
                                .fg(t.subtle)
                                .add_modifier(Modifier::DIM | Modifier::ITALIC),
                        ),
                    ]));
                    first = false;
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", line),
                        Style::default()
                            .fg(t.subtle)
                            .add_modifier(Modifier::DIM | Modifier::ITALIC),
                    )));
                }
            }
            lines
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
                " · ↑{} ↓{} ({}% cached) · {}",
                fmt_tokens(app.total_input_tokens),
                fmt_tokens(app.total_output_tokens),
                cache_pct,
                piko_api::format_cost(app.total_cost_usd)
            )
        } else {
            format!(
                " · ↑{} ↓{} · {}",
                fmt_tokens(app.total_input_tokens),
                fmt_tokens(app.total_output_tokens),
                piko_api::format_cost(app.total_cost_usd)
            )
        }
    } else if app.total_cost_usd > 0.0 {
        format!(" · {}", piko_api::format_cost(app.total_cost_usd))
    } else {
        String::new()
    };

    // Rate limit display: show countdown if active, clear once expired.
    let rate_limit_part = if let Some(until) = app.rate_limit_until {
        let now = std::time::Instant::now();
        if until > now {
            let secs = (until - now).as_secs() + 1;
            format!(
                " · ⏸ rate limited · resets in {}m {:02}s",
                secs / 60,
                secs % 60
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let rate_limit_color = if app
        .rate_limit_until
        .map(|u| u > std::time::Instant::now())
        .unwrap_or(false)
    {
        t.warning
    } else {
        t.subtle
    };

    let plan_mode_part = if app.plan_mode.load(Ordering::SeqCst) {
        " [PLAN MODE]".to_string()
    } else {
        String::new()
    };

    let left_spans = vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(state_color)),
        Span::styled(token_part, Style::default().fg(t.subtle)),
        Span::styled(
            rate_limit_part,
            Style::default()
                .fg(rate_limit_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            plan_mode_part,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
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
        AppState::AskingPlanModeExit => ("◑".to_string(), Color::Yellow),
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

    let text_style = Style::default().fg(t.text);
    let cursor = if app.state == AppState::Running {
        "█"
    } else {
        ""
    };

    let lines: Vec<Line> = if app.input.is_empty() {
        vec![Line::from(vec![
            Span::styled(format!("{} ", POINTER), pointer_style),
            Span::styled("Ask anything...", Style::default().fg(t.inactive)),
            Span::raw(cursor),
        ])]
    } else {
        let input_with_cursor = format!(
            "{}{}{}",
            &app.input[..app.cursor_pos],
            cursor,
            &app.input[app.cursor_pos..]
        );
        input_with_cursor
            .split('\n')
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled(format!("{} ", POINTER), pointer_style),
                        Span::styled(line.to_owned(), text_style),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(line.to_owned(), text_style),
                    ])
                }
            })
            .collect()
    };

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
        Paragraph::new(Text::from(lines))
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

// ── Plan mode exit dialog ─────────────────────────────────────────────────────

fn render_plan_mode_exit_dialog(frame: &mut Frame, area: ratatui::layout::Rect, t: &Theme) {
    let lines = vec![Line::from(vec![
        Span::styled(
            "Agent wants to exit plan mode and begin making changes. ",
            Style::default().fg(t.text),
        ),
        Span::styled(
            "Allow?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Line::from(vec![
            Span::styled(" Exit Plan Mode  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "(y)",
                Style::default().fg(t.success).add_modifier(Modifier::BOLD),
            ),
            Span::styled("es  ", Style::default().fg(t.subtle)),
            Span::styled(
                "(n)",
                Style::default().fg(t.error).add_modifier(Modifier::BOLD),
            ),
            Span::styled("o ", Style::default().fg(t.subtle)),
        ]));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spans_text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    // ── system_icon ────────────────────────────────────────────────────────────

    #[test]
    fn system_icon_running_tool() {
        let t = crate::theme::by_name("dark");
        let (icon, _) = system_icon("[bash] running...", t);
        assert_eq!(icon, "◆");
    }

    #[test]
    fn system_icon_error() {
        let t = crate::theme::by_name("dark");
        let (icon, _) = system_icon("[bash] error: something", t);
        assert_eq!(icon, "✗");
    }

    #[test]
    fn system_icon_permission() {
        let t = crate::theme::by_name("dark");
        let (icon, _) = system_icon("[permission] bash → Allow", t);
        assert_eq!(icon, "◈");
    }

    #[test]
    fn system_icon_default() {
        let t = crate::theme::by_name("dark");
        let (icon, _) = system_icon("some random message", t);
        assert_eq!(icon, "·");
    }

    // ── block_line_spans ───────────────────────────────────────────────────────

    fn ts() -> Style {
        Style::default().fg(ratatui::style::Color::White)
    }
    fn cs() -> Style {
        Style::default().fg(ratatui::style::Color::Blue)
    }

    #[test]
    fn heading_h1_bold_underline() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("# Hello", ts(), cs(), t);
        assert_eq!(spans_text(&spans), "Hello");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn heading_h2_bold_only() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("## World", ts(), cs(), t);
        assert_eq!(spans_text(&spans), "World");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn unordered_list_dash() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("- item", ts(), cs(), t);
        assert!(spans_text(&spans).starts_with('•'));
        assert!(spans_text(&spans).contains("item"));
    }

    #[test]
    fn unordered_list_star() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("* item", ts(), cs(), t);
        assert!(spans_text(&spans).starts_with('•'));
    }

    #[test]
    fn ordered_list() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("1. first", ts(), cs(), t);
        let text = spans_text(&spans);
        assert!(text.contains("1."));
        assert!(text.contains("first"));
    }

    #[test]
    fn blockquote() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("> quoted", ts(), cs(), t);
        assert!(spans_text(&spans).contains('│'));
        assert!(spans_text(&spans).contains("quoted"));
    }

    #[test]
    fn horizontal_rule() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("---", ts(), cs(), t);
        assert!(spans_text(&spans).contains('─'));
    }

    #[test]
    fn plain_line_passthrough() {
        let t = crate::theme::by_name("dark");
        let spans = block_line_spans("hello world", ts(), cs(), t);
        assert_eq!(spans_text(&spans), "hello world");
    }

    // ── message_to_lines scroll viewport ──────────────────────────────────────

    #[test]
    fn message_to_lines_user_has_bg() {
        let t = crate::theme::by_name("dark");
        let lines = message_to_lines(&MessageRole::User, "hello", t);
        assert!(!lines.is_empty());
        let first_span = &lines[0].spans[0];
        assert_eq!(first_span.style.bg, Some(t.user_msg_bg));
    }

    #[test]
    fn message_to_lines_assistant_starts_with_circle() {
        let t = crate::theme::by_name("dark");
        let lines = message_to_lines(&MessageRole::Assistant, "hi there", t);
        assert!(!lines.is_empty());
        assert!(lines[0].spans[0].content.contains('⏺'));
    }

    #[test]
    fn message_to_lines_assistant_markdown_heading() {
        let t = crate::theme::by_name("dark");
        let lines = message_to_lines(&MessageRole::Assistant, "# Title\ntext", t);
        let all_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(all_text.contains("Title"));
        assert!(!all_text.contains("# Title"));
    }
}
