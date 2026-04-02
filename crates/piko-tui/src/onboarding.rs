/// First-run onboarding: theme selection screen.
///
/// Renders a full-screen theme picker using ratatui + crossterm.
/// Returns the name of the chosen theme so the caller can persist it.
use crate::theme::{Theme, ALL_THEMES};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::stdout;

/// Run the onboarding theme picker. Blocks until the user confirms a choice.
/// Returns the selected theme name (e.g. `"dark"`).
pub fn run_theme_picker() -> Result<&'static str> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut selected: usize = 0; // index into ALL_THEMES

    let chosen = loop {
        terminal.draw(|f| draw_picker(f, selected))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    // Quit without choosing (keep default)
                    (KeyCode::Char('c'), KeyModifiers::CONTROL)
                    | (KeyCode::Char('q'), KeyModifiers::NONE) => {
                        break ALL_THEMES[0].name; // "dark"
                    }
                    // Navigate up
                    (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                        selected = selected.saturating_sub(1);
                    }
                    // Navigate down
                    (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                        if selected + 1 < ALL_THEMES.len() {
                            selected += 1;
                        }
                    }
                    // Confirm
                    (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => {
                        break ALL_THEMES[selected].name;
                    }
                    _ => {}
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(chosen)
}

fn draw_picker(frame: &mut Frame, selected: usize) {
    let t = ALL_THEMES[selected]; // live preview using selected theme

    // Fill background with the selected theme's status_bg colour
    let full = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(t.status_bg)),
        full,
    );

    // Centre a dialog box: max 60 wide, enough rows for all themes + chrome
    let dialog_w = full.width.min(62);
    let dialog_h = (ALL_THEMES.len() as u16) + 10; // items + header + footer
    let dialog_h = dialog_h.min(full.height);

    let x = full.x + (full.width.saturating_sub(dialog_w)) / 2;
    let y = full.y + (full.height.saturating_sub(dialog_h)) / 2;
    let dialog = Rect::new(x, y, dialog_w, dialog_h);

    frame.render_widget(Clear, dialog);

    // Outer box
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.claude))
        .title(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                "pikoclaw",
                Style::default().fg(t.claude).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — choose a theme ", Style::default().fg(t.inactive)),
        ]))
        .style(Style::default().bg(t.status_bg));
    frame.render_widget(outer, dialog);

    // Inner layout: intro | list | swatch row | footer hint
    let inner = Rect::new(
        dialog.x + 2,
        dialog.y + 2,
        dialog.width.saturating_sub(4),
        dialog.height.saturating_sub(4),
    );

    let row_count = ALL_THEMES.len() as u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),         // intro text
            Constraint::Length(row_count), // theme list
            Constraint::Length(2),         // colour swatch preview
            Constraint::Min(1),            // footer
        ])
        .split(inner);

    // ── Intro ──────────────────────────────────────────────────────────────
    let intro = Paragraph::new(Line::from(vec![
        Span::styled(
            "Welcome! ",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Select a colour theme to get started.",
            Style::default().fg(t.inactive),
        ),
    ]));
    frame.render_widget(intro, chunks[0]);

    // ── Theme list ─────────────────────────────────────────────────────────
    let items: Vec<ListItem> = ALL_THEMES
        .iter()
        .enumerate()
        .map(|(i, th)| {
            let is_sel = i == selected;
            let prefix = if is_sel { "❯ " } else { "  " };
            let style = if is_sel {
                Style::default().fg(t.claude).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.inactive)
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix.to_string(), style),
                Span::styled(th.label.to_string(), style),
                Span::styled(format!("  ({})", th.name), Style::default().fg(t.subtle)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), chunks[1]);

    // ── Colour swatch of the currently previewed theme ─────────────────────
    render_swatch(frame, t, chunks[2]);

    // ── Footer ─────────────────────────────────────────────────────────────
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "↑↓",
            Style::default()
                .fg(t.permission)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" navigate  ", Style::default().fg(t.subtle)),
        Span::styled(
            "Enter",
            Style::default().fg(t.success).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" confirm  ", Style::default().fg(t.subtle)),
        Span::styled("To change later: ", Style::default().fg(t.subtle)),
        Span::styled("/theme", Style::default().fg(t.claude)),
    ]))
    .alignment(Alignment::Left);
    frame.render_widget(footer, chunks[3]);
}

/// Render a compact row of colour swatches for `t`.
fn render_swatch(frame: &mut Frame, t: &Theme, area: Rect) {
    // Show a labelled block of colour squares side-by-side
    let swatches: &[(Color, &str)] = &[
        (t.claude, "brand "),
        (t.success, "ok    "),
        (t.error, "err   "),
        (t.warning, "warn  "),
        (t.permission, "info  "),
        (t.text, "text  "),
    ];

    let n = swatches.len() as u16;
    if area.width < n * 8 {
        return; // too narrow to render
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Length(area.width / n); swatches.len()])
        .split(area);

    for (i, (color, label)) in swatches.iter().enumerate() {
        let block = Paragraph::new(Line::from(Span::styled(
            format!(" {} ", label),
            Style::default().fg(t.status_bg).bg(*color),
        )));
        frame.render_widget(block, cols[i]);
    }
}
