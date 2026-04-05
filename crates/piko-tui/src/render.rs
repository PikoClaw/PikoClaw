use crate::app::{App, AppState, ChatMessage, MessageRole};
use crate::highlight::{highlight_code, parse_inline_spans, parse_segments, Segment};
use crate::slash_menu::TypeaheadSource;
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

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let truncated: String = text.chars().take(max_chars - 1).collect();
    format!("{truncated}…")
}

/// Compute how many terminal rows the input bar needs given the current text
/// and terminal width.
///
/// Accounts for:
/// - 2 border rows (rounded box top + bottom)
/// - The `"> "` 2-char prefix on the first line of each logical (newline-split) line
/// - ratatui `Wrap` breaking at inner width (`area.width - 2`)
/// - A cap of 10 rows so the input never consumes more than ~a third of a
///   normal 30-row terminal
fn input_bar_height(input: &str, area_width: u16) -> u16 {
    // Inner width = terminal width minus left+right borders
    let inner = area_width.saturating_sub(2) as usize;
    if inner == 0 {
        return 3;
    }

    // Each logical line (split by \n) occupies ceil((2 + char_count) / inner) visual rows.
    // The 2 accounts for the "> " / "  " prefix the widget draws on every line.
    let visual_rows: usize = input
        .split('\n')
        .map(|seg| {
            let char_count = 2 + seg.chars().count(); // 2 = prefix width
            char_count.div_ceil(inner).max(1)
        })
        .sum::<usize>()
        .max(1); // always at least one row even when input is empty

    // 2 border rows + content rows; clamp to [3, 10]
    (visual_rows as u16 + 2).clamp(3, 10)
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let t = app.theme;

    // Paint the full-frame background so terminal default doesn't bleed through.
    frame.render_widget(Block::default().style(Style::default().bg(t.bg)), area);

    // Input bar height grows with the text so long lines wrap instead of clipping.
    let input_height = input_bar_height(&app.input, area.width);

    let suggestions_height = if app.state == AppState::Running && !app.slash_suggestions.is_empty()
    {
        app.slash_suggestions.len().min(5) as u16
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(input_height),
            Constraint::Length(suggestions_height),
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
        AppState::SelectingProvider => {
            render_input_bar(frame, app, chunks[2], t);
            render_connect_dialog(frame, app, area, t);
        }
        AppState::EnteringApiKey => {
            render_input_bar(frame, app, chunks[2], t);
            render_api_key_dialog(frame, app, area, t);
        }
        _ => {
            render_input_bar(frame, app, chunks[2], t);
            render_prompt_suggestions(frame, app, chunks[3], t);
        }
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

fn render_messages(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let area_width = area.width as usize;
    let height = area.height as usize;

    let all_lines: Vec<Line> = app
        .messages
        .iter()
        .flat_map(|msg| message_to_lines(msg, t, area_width))
        .collect();
    let all_lines = if all_lines.is_empty() {
        vec![Line::from("")]
    } else {
        all_lines
    };

    let total = all_lines.len();
    app.last_total_lines.set(total);
    app.last_frame_height.set(height);

    let max_scroll = total.saturating_sub(height);
    let start = if app.follow_bottom {
        max_scroll
    } else {
        app.scroll.min(max_scroll)
    };
    let end = (start + height).min(total);

    let visible: Vec<Line> = all_lines[start..end].to_vec();
    frame.render_widget(
        Paragraph::new(Text::from(visible)).style(Style::default().bg(t.bg)),
        area,
    );
}

/// Word-wrap a single logical line into multiple terminal rows with hanging indent.
///
/// The prefix width of the line (bullet, indent, etc.) is detected from the leading spans
/// and used as the continuation indent so wrapped text aligns under the content, not column 0.
fn word_wrap(spans: Vec<Span<'static>>, area_width: usize) -> Vec<Line<'static>> {
    if area_width == 0 {
        return vec![Line::from(spans)];
    }
    let total_w: usize = spans.iter().map(|s| s.width()).sum();
    if total_w <= area_width {
        return vec![Line::from(spans)];
    }

    // Measure the prefix (non-content leading spans) to compute hanging indent.
    // Leading spans that are pure whitespace or punctuation markers ("• ", "│ ", "N. ", etc.)
    // form the prefix; content text starts after them.
    let prefix_w = leading_prefix_width(&spans);
    let cont_indent: String = " ".repeat(prefix_w);

    // Tokenise into (text, style, is_whitespace) so we can pack words onto lines.
    let mut tokens: Vec<(String, Style, bool)> = Vec::new();
    for span in &spans {
        let style = span.style;
        let mut buf = String::new();
        let mut buf_ws = false;
        for ch in span.content.chars() {
            let ws = ch == ' ' || ch == '\t';
            if buf.is_empty() {
                buf_ws = ws;
            } else if ws != buf_ws {
                tokens.push((buf.clone(), style, buf_ws));
                buf.clear();
                buf_ws = ws;
            }
            buf.push(ch);
        }
        if !buf.is_empty() {
            tokens.push((buf, style, buf_ws));
        }
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut cur_w: usize = 0;
    let mut first = true;

    for (text, style, is_ws) in tokens {
        let tok_w = text.chars().count();
        if is_ws {
            if cur_w > 0 && cur_w + tok_w <= area_width {
                cur.push(Span::styled(text, style));
                cur_w += tok_w;
            }
            continue;
        }
        if cur_w > 0 && cur_w + tok_w > area_width {
            result.push(Line::from(std::mem::take(&mut cur)));
            cur.push(Span::raw(cont_indent.clone()));
            cur_w = prefix_w;
            first = false;
        }
        cur.push(Span::styled(text, style));
        cur_w += tok_w;
    }
    if !cur.is_empty() {
        result.push(Line::from(cur));
    }
    if result.is_empty() {
        result.push(Line::from(spans));
    }
    let _ = first;
    result
}

/// Returns the display width of the "prefix" portion of a line's spans.
///
/// A prefix is the leading non-content part: whitespace, bullet markers, box-drawing chars.
/// This is used to compute the hanging indent for wrapped continuation lines.
fn leading_prefix_width(spans: &[Span<'static>]) -> usize {
    let mut w = 0;
    for span in spans {
        let content = span.content.as_ref();
        // Treat a span as part of the prefix if ALL its characters are
        // whitespace or known marker characters (•, │, ⏺, ─, box-drawing range).
        let is_prefix = content.chars().all(|c| {
            c == ' ' || c == '\t' || c == '•' || c == '│' || c == '⏺'
                || c == '─' || c == '╭' || c == '╰' || c == '╯' || c == '╮'
                || c == '┌' || c == '└' || c == '┐' || c == '┘'
                // ordered list: digits and ". "
                || c.is_ascii_digit() || c == '.'
        });
        if is_prefix {
            w += content.chars().count();
        } else {
            break;
        }
    }
    w
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

fn message_to_lines(msg: &ChatMessage, t: &Theme, area_width: usize) -> Vec<Line<'static>> {
    let role = &msg.role;
    let content: &str = &msg.content;
    match role {
        MessageRole::ToolCall => {
            let Some(info) = &msg.tool_info else {
                return vec![];
            };
            let mut lines: Vec<Line> = Vec::new();

            let elapsed = info
                .completed_at
                .unwrap_or_else(std::time::Instant::now)
                .saturating_duration_since(info.started_at);
            let elapsed_text = format_elapsed(elapsed);
            let is_running = info.result.is_none();
            let icon = if is_running {
                spinner_frame()
            } else if info.result.as_ref().is_some_and(|r| r.is_error) {
                "✗".to_string()
            } else {
                "✓".to_string()
            };
            let icon_style = if is_running {
                Style::default().fg(t.permission)
            } else if info.result.as_ref().is_some_and(|r| r.is_error) {
                Style::default().fg(t.error)
            } else {
                Style::default().fg(t.success)
            };
            let name_style = if is_running {
                Style::default()
                    .fg(t.permission)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.inactive).add_modifier(Modifier::DIM)
            };
            let preview_style = Style::default().fg(t.inactive).add_modifier(Modifier::DIM);
            let indent = "  ";
            let meta_prefix = if info.args_display.is_empty() {
                String::new()
            } else {
                format!(" · {}", info.args_display.replace('\n', " "))
            };
            let left_plain = format!("{} {}{}", icon, info.display_name, meta_prefix);
            let time_width = elapsed_text.chars().count();
            let left_width = left_plain.chars().count();

            if area_width > left_width + time_width + 1 {
                let padding = area_width - left_width - time_width;
                let mut spans = vec![
                    Span::styled(format!("{} ", icon), icon_style),
                    Span::styled(info.display_name.clone(), name_style),
                ];
                if !info.args_display.is_empty() {
                    spans.push(Span::styled(" · ", preview_style));
                    spans.push(Span::styled(
                        info.args_display.replace('\n', " "),
                        preview_style,
                    ));
                }
                spans.push(Span::raw(" ".repeat(padding)));
                spans.push(Span::styled(elapsed_text.clone(), preview_style));
                lines.push(Line::from(spans));
            } else {
                let mut header = vec![
                    Span::styled(format!("{} ", icon), icon_style),
                    Span::styled(info.display_name.clone(), name_style),
                ];
                if !info.args_display.is_empty() {
                    header.push(Span::styled(" · ", preview_style));
                    header.push(Span::styled(
                        info.args_display.replace('\n', " "),
                        preview_style,
                    ));
                }
                lines.extend(word_wrap(header, area_width));
                lines.push(Line::from(Span::styled(
                    format!("{indent}{elapsed_text}"),
                    preview_style,
                )));
            }

            if let Some(result) = info
                .result
                .as_ref()
                .filter(|r| r.is_error && !r.text.is_empty())
            {
                let result_color = if result.is_error { t.error } else { t.inactive };
                lines.extend(result.text.lines().map(|line| {
                    Line::from(Span::styled(
                        format!("{indent}{line}"),
                        Style::default().fg(result_color),
                    ))
                }));
            }
            lines.push(Line::from(""));
            lines
        }

        MessageRole::User => {
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(Span::styled(
                "╭─ You ".to_owned(),
                Style::default().fg(t.subtle),
            )));
            for line in content.lines() {
                let spans = vec![
                    Span::styled("│ ", Style::default().fg(t.subtle)),
                    Span::styled(
                        line.to_owned(),
                        Style::default().fg(t.text).add_modifier(Modifier::BOLD),
                    ),
                ];
                lines.extend(word_wrap(spans, area_width));
            }
            lines.push(Line::from(Span::styled(
                "╰─".to_owned(),
                Style::default().fg(t.subtle),
            )));
            lines.push(Line::from(""));
            lines
        }

        MessageRole::Assistant => {
            let mut lines: Vec<Line> = Vec::new();
            let mut first_line = true;
            let text_style = Style::default().fg(t.text);
            let code_style = Style::default().fg(t.permission);

            // Helper: prepend the ⏺ or indent prefix to spans.
            fn with_prefix(
                content: Vec<Span<'static>>,
                first_line: &mut bool,
                claude_color: Color,
            ) -> Vec<Span<'static>> {
                if *first_line {
                    *first_line = false;
                    let mut s = vec![Span::styled(
                        format!("{} ", BLACK_CIRCLE),
                        Style::default().fg(claude_color),
                    )];
                    s.extend(content);
                    s
                } else {
                    let mut s = vec![Span::raw("  ")];
                    s.extend(content);
                    s
                }
            }

            for segment in parse_segments(content) {
                match segment {
                    Segment::Text(text) => {
                        // pending_bullet: holds spans for a bullet line whose content
                        // arrived on the next line (model multi-line **bold** pattern).
                        let mut pending_bullet: Option<Vec<Span<'static>>> = None;

                        for raw_line in text.lines() {
                            let content_spans =
                                block_line_spans(raw_line, text_style, code_style, t);

                            if content_spans.is_empty() {
                                // Flush a lone pending bullet before the blank line.
                                if let Some(b) = pending_bullet.take() {
                                    let spans = with_prefix(b, &mut first_line, t.claude);
                                    lines.extend(word_wrap(spans, area_width));
                                }
                                if !first_line {
                                    lines.push(Line::from(""));
                                }
                                continue;
                            }

                            // A "lone bullet" is a bullet marker with no following text
                            // (e.g. "- **" where parse_inline_spans consumed the markers
                            // but produced no visible content).
                            let is_lone_bullet = content_spans.len() == 1
                                && content_spans[0].content.as_ref() == "• ";

                            if is_lone_bullet {
                                // Flush any previous pending bullet first.
                                if let Some(b) = pending_bullet.take() {
                                    let spans = with_prefix(b, &mut first_line, t.claude);
                                    lines.extend(word_wrap(spans, area_width));
                                }
                                pending_bullet = Some(content_spans);
                                continue;
                            }

                            // Normal content: merge with a pending bullet if present.
                            let merged = if let Some(mut b) = pending_bullet.take() {
                                b.extend(content_spans);
                                b
                            } else {
                                content_spans
                            };

                            let spans = with_prefix(merged, &mut first_line, t.claude);
                            lines.extend(word_wrap(spans, area_width));
                        }

                        // Flush any trailing lone bullet.
                        if let Some(b) = pending_bullet.take() {
                            let spans = with_prefix(b, &mut first_line, t.claude);
                            lines.extend(word_wrap(spans, area_width));
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

    let scroll_part = if !app.follow_bottom {
        let total = app.last_total_lines.get();
        let height = app.last_frame_height.get();
        let max_start = total.saturating_sub(height);
        if max_start > 0 {
            let pct = 100 - (app.scroll * 100 / max_start.max(1));
            format!(" · ↑ {}%", pct.min(100))
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let left_spans = vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(state_color)),
        Span::styled(token_part, Style::default().fg(t.subtle)),
        Span::styled(
            scroll_part,
            Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
        ),
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
        AppState::SelectingProvider => ("⇄".to_string(), t.permission),
        AppState::EnteringApiKey => ("⌘".to_string(), t.permission),
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

fn spinner_frame() -> String {
    let frame = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 100) as usize
        % 10;
    const TOOL_SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    TOOL_SPINNER[frame].to_string()
}

fn format_elapsed(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();
    if secs == 0 {
        format!("0.{:01}s", elapsed.subsec_millis() / 100)
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
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
        render_input_lines(
            &app.input,
            app.cursor_pos,
            cursor,
            pointer_style,
            text_style,
            t,
        )
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

fn render_input_lines<'a>(
    input: &'a str,
    cursor_pos: usize,
    cursor: &'a str,
    pointer_style: Style,
    text_style: Style,
    t: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut offset = 0usize;
    for (i, line) in input.split('\n').enumerate() {
        let line_end = offset + line.len();
        let mut spans = if i == 0 {
            vec![Span::styled(format!("{} ", POINTER), pointer_style)]
        } else {
            vec![Span::raw("  ")]
        };
        spans.extend(render_input_line_segments(
            input, offset, line_end, cursor_pos, cursor, text_style, t,
        ));
        lines.push(Line::from(spans));
        offset = line_end + 1;
    }
    if input.ends_with('\n') {
        let prefix = vec![Span::raw("  ")];
        let mut spans = prefix;
        if cursor_pos == input.len() && !cursor.is_empty() {
            spans.push(Span::raw(cursor.to_string()));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn render_input_line_segments(
    input: &str,
    start: usize,
    end: usize,
    cursor_pos: usize,
    cursor: &str,
    text_style: Style,
    t: &Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut pos = start;
    while pos < end {
        if pos == cursor_pos && !cursor.is_empty() {
            spans.push(Span::raw(cursor.to_string()));
        }
        if let Some((chip_end, selected)) = chip_segment_at(input, pos, cursor_pos) {
            spans.push(Span::styled(
                input[pos..chip_end].to_string(),
                if selected {
                    Style::default().fg(t.bg).bg(t.text)
                } else {
                    text_style
                },
            ));
            pos = chip_end;
            continue;
        }
        let next_chip = next_chip_start(input, pos, end);
        let next_cursor = if cursor_pos > pos && cursor_pos < end {
            cursor_pos
        } else {
            end
        };
        let boundary = next_chip.min(next_cursor).min(end);
        if boundary > pos {
            spans.push(Span::styled(input[pos..boundary].to_string(), text_style));
        }
        pos = boundary;
    }
    if cursor_pos == end && !cursor.is_empty() {
        spans.push(Span::raw(cursor.to_string()));
    }
    spans
}

fn next_chip_start(input: &str, start: usize, end: usize) -> usize {
    input[start..end]
        .find('[')
        .map(|idx| start + idx)
        .unwrap_or(end)
}

fn chip_segment_at(input: &str, start: usize, cursor_pos: usize) -> Option<(usize, bool)> {
    let after = &input[start..];
    let close = after.find(']')?;
    let candidate = &after[..=close];
    if !is_render_chip(candidate) {
        return None;
    }
    let end = start + candidate.len();
    Some((end, cursor_pos == start || cursor_pos == end))
}

fn is_render_chip(s: &str) -> bool {
    ((s.starts_with("[Pasted text #") || s.starts_with("[...Truncated text #"))
        || s.starts_with("[Image #"))
        && s.ends_with(']')
}

fn render_prompt_suggestions(frame: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let suggestions = &app.slash_suggestions;
    if suggestions.is_empty() || area.height == 0 {
        return;
    }

    let selected = app.slash_suggestion_index.unwrap_or(0);
    let max_visible = area.height as usize;
    let start = selected
        .saturating_sub(max_visible / 2)
        .min(suggestions.len().saturating_sub(max_visible));
    let end = (start + max_visible).min(suggestions.len());
    let label_width = area.width.saturating_div(3).max(12) as usize;
    let lines: Vec<Line> = suggestions[start..end]
        .iter()
        .enumerate()
        .map(|(row, suggestion)| {
            let is_selected = start + row == selected;
            let accent_style = if is_selected {
                Style::default().fg(t.claude).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.inactive)
            };
            let label_style = if is_selected {
                Style::default().fg(t.claude).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text)
            };
            let detail_style = if is_selected {
                Style::default().fg(t.claude)
            } else {
                Style::default().fg(t.inactive)
            };
            let mut spans = vec![Span::styled(
                if is_selected { "› " } else { "  " },
                accent_style,
            )];
            match suggestion.source {
                TypeaheadSource::SlashCommand => {
                    let display_name = truncate_text(&suggestion.text, label_width);
                    spans.push(Span::styled(
                        format!("{display_name:<width$}", width = label_width),
                        label_style,
                    ));
                    spans.push(Span::styled(" [cmd] ", Style::default().fg(t.subtle)));
                    if !suggestion.description.is_empty() {
                        spans.push(Span::styled(
                            truncate_text(
                                &suggestion.description,
                                area.width.saturating_sub(label_width as u16 + 10) as usize,
                            ),
                            detail_style,
                        ));
                    }
                }
            }
            Line::from(spans)
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).style(Style::default().bg(t.bg)), area);
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

fn render_connect_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let dialog = centered_rect(64, 10, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0))),
        area,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.claude))
        .title(Span::styled(
            " Connect a Provider ",
            Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
        ));

    let mut lines = vec![Line::from(Span::styled(
        "Choose a provider for this session",
        Style::default().fg(t.inactive),
    ))];

    for (idx, option) in app.connect_dialog.options.iter().enumerate() {
        let selected = idx == app.connect_dialog.selected_index;
        let marker = if selected { "›" } else { " " };
        let name_style = if selected {
            Style::default().fg(t.claude).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.text)
        };
        let desc_style = if selected {
            Style::default().fg(t.claude)
        } else {
            Style::default().fg(t.inactive)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), Style::default().fg(t.claude)),
            Span::styled(option.label, name_style),
            Span::raw("  "),
            Span::styled(option.description, desc_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter to continue, Esc to cancel",
        Style::default().fg(t.subtle),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(t.bg))
            .wrap(Wrap { trim: false }),
        dialog,
    );
}

fn render_api_key_dialog(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, t: &Theme) {
    let Some(dialog_state) = app.api_key_dialog.as_ref() else {
        return;
    };

    let dialog = centered_rect(64, 9, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0))),
        area,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.permission))
        .title(Span::styled(
            format!(" Connect {} ", dialog_state.provider_label),
            Style::default()
                .fg(t.permission)
                .add_modifier(Modifier::BOLD),
        ));

    let masked = if dialog_state.input.is_empty() {
        "Paste your API key here...".to_string()
    } else {
        let visible = dialog_state.input.chars().count().saturating_sub(4);
        let tail: String = dialog_state.input.chars().skip(visible).collect();
        format!("{}{}", "•".repeat(visible), tail)
    };

    let lines = vec![
        Line::from(Span::styled("API Key", Style::default().fg(t.inactive))),
        Line::from(vec![
            Span::styled(
                masked,
                if dialog_state.input.is_empty() {
                    Style::default().fg(t.subtle)
                } else {
                    Style::default().fg(t.text)
                },
            ),
            Span::styled("█", Style::default().fg(t.permission)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter to save, Esc to go back",
            Style::default().fg(t.subtle),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(t.bg))
            .wrap(Wrap { trim: false }),
        dialog,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let popup_width = width.min(area.width.saturating_sub(4));
    let popup_height = height.min(area.height.saturating_sub(4));
    Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
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
    fn message_to_lines_user_has_box() {
        let t = crate::theme::by_name("dark");
        let msg = ChatMessage::text(MessageRole::User, "hello");
        let lines = message_to_lines(&msg, t, 80);
        assert!(!lines.is_empty());
        let first_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first_text.contains('╭') || first_text.contains("You"));
    }

    #[test]
    fn message_to_lines_assistant_starts_with_circle() {
        let t = crate::theme::by_name("dark");
        let msg = ChatMessage::text(MessageRole::Assistant, "hi there");
        let lines = message_to_lines(&msg, t, 80);
        assert!(!lines.is_empty());
        assert!(lines[0].spans[0].content.contains('⏺'));
    }

    #[test]
    fn message_to_lines_assistant_markdown_heading() {
        let t = crate::theme::by_name("dark");
        let msg = ChatMessage::text(MessageRole::Assistant, "# Title\ntext");
        let lines = message_to_lines(&msg, t, 80);
        let all_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(all_text.contains("Title"));
        assert!(!all_text.contains("# Title"));
    }
}
