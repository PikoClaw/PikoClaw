/// Outcome of the provider picker shown on fresh startup.
pub enum ProviderChoice {
    /// User chose Anthropic — caller should run the OAuth/browser login flow.
    Anthropic,
    /// User chose another provider and entered an API key.
    ApiKey {
        provider_id: String,
        provider_label: String,
        base_url: String,
        api_key: String,
        use_bearer: bool,
    },
}

impl ProviderChoice {
    pub fn provider_id(&self) -> Option<&str> {
        match self { Self::ApiKey { provider_id, .. } => Some(provider_id), _ => None }
    }
    pub fn provider_label(&self) -> Option<&str> {
        match self { Self::ApiKey { provider_label, .. } => Some(provider_label), _ => None }
    }
    pub fn base_url(&self) -> Option<&str> {
        match self { Self::ApiKey { base_url, .. } => Some(base_url), _ => None }
    }
    pub fn api_key(&self) -> Option<&str> {
        match self { Self::ApiKey { api_key, .. } => Some(api_key), _ => None }
    }
    pub fn use_bearer(&self) -> bool {
        match self { Self::ApiKey { use_bearer, .. } => *use_bearer, _ => false }
    }
}

struct ProviderOption {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    base_url: &'static str,
    use_bearer: bool,
}

static PROVIDERS: &[ProviderOption] = &[
    ProviderOption {
        id: "anthropic",
        label: "Anthropic / Claude",
        description: "Sign in with your Claude account (browser login)",
        base_url: "https://api.anthropic.com",
        use_bearer: false,
    },
    ProviderOption {
        id: "openai",
        label: "OpenAI",
        description: "GPT models — paste your OpenAI API key",
        base_url: "https://api.openai.com",
        use_bearer: true,
    },
    ProviderOption {
        id: "google",
        label: "Google",
        description: "Gemini models — paste your Google AI API key",
        base_url: "https://generativelanguage.googleapis.com",
        use_bearer: false,
    },
    ProviderOption {
        id: "openrouter",
        label: "OpenRouter",
        description: "100+ models via openrouter.ai — paste your API key",
        base_url: "https://openrouter.ai/api",
        use_bearer: true,
    },
];

/// Show a full-screen provider selection dialog on fresh startup.
/// Returns the user's choice so `main` can set up credentials appropriately.
pub fn run_provider_picker(theme_name: &str) -> Result<ProviderChoice> {
    let t = crate::theme::ALL_THEMES
        .iter()
        .find(|th| th.name == theme_name)
        .copied()
        .unwrap_or(crate::theme::ALL_THEMES[0]);

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    #[derive(PartialEq)]
    enum Screen { SelectProvider, EnterKey }

    let mut screen = Screen::SelectProvider;
    let mut selected: usize = 0;
    let mut key_input = String::new();

    let choice = loop {
        let sel = selected;
        let ki = key_input.clone();
        match screen {
            Screen::SelectProvider => terminal.draw(|f| draw_provider_list(f, t, sel))?,
            Screen::EnterKey      => terminal.draw(|f| draw_key_entry(f, t, &PROVIDERS[sel], &ki))?,
        };

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match screen {
                    Screen::SelectProvider => match (key.code, key.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL)
                        | (KeyCode::Esc, _) => break ProviderChoice::Anthropic,
                        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                            selected = selected.saturating_sub(1);
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                            if selected + 1 < PROVIDERS.len() {
                                selected += 1;
                            }
                        }
                        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => {
                            if PROVIDERS[selected].id == "anthropic" {
                                break ProviderChoice::Anthropic;
                            } else {
                                key_input.clear();
                                screen = Screen::EnterKey;
                            }
                        }
                        _ => {}
                    },
                    Screen::EnterKey => match (key.code, key.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => break ProviderChoice::Anthropic,
                        (KeyCode::Esc, _) => {
                            screen = Screen::SelectProvider;
                            key_input.clear();
                        }
                        (KeyCode::Backspace, _) => { key_input.pop(); }
                        (KeyCode::Enter, _) => {
                            let k = key_input.trim().to_string();
                            if !k.is_empty() {
                                let p = &PROVIDERS[selected];
                                break ProviderChoice::ApiKey {
                                    provider_id: p.id.to_string(),
                                    provider_label: p.label.to_string(),
                                    base_url: p.base_url.to_string(),
                                    api_key: k,
                                    use_bearer: p.use_bearer,
                                };
                            }
                        }
                        (KeyCode::Char(c), _) => { key_input.push(c); }
                        _ => {}
                    },
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(choice)
}

fn draw_provider_list(frame: &mut Frame, t: &crate::theme::Theme, selected: usize) {
    let full = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(t.bg)), full);

    let n_providers = PROVIDERS.len() as u16;
    let dialog_w = full.width.min(70);
    let dialog_h = (n_providers + 8).min(full.height);
    let x = full.x + (full.width.saturating_sub(dialog_w)) / 2;
    let y = full.y + (full.height.saturating_sub(dialog_h)) / 2;
    let dialog = Rect::new(x, y, dialog_w, dialog_h);

    frame.render_widget(Clear, dialog);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.claude))
        .title(Line::from(vec![
            Span::styled(" pikoclaw ", Style::default().fg(t.claude).add_modifier(Modifier::BOLD)),
            Span::styled("— choose a provider ", Style::default().fg(t.inactive)),
        ]))
        .style(Style::default().bg(t.bg));
    frame.render_widget(outer, dialog);

    let inner = Rect::new(dialog.x + 2, dialog.y + 2, dialog.width.saturating_sub(4), dialog.height.saturating_sub(4));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(n_providers),
            Constraint::Min(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("No credentials found. ", Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
            Span::styled("Select a provider to get started.", Style::default().fg(t.inactive)),
        ])),
        chunks[0],
    );

    let items: Vec<ListItem> = PROVIDERS.iter().enumerate().map(|(i, p)| {
        let is_sel = i == selected;
        let prefix = if is_sel { "❯ " } else { "  " };
        let name_style = if is_sel {
            Style::default().fg(t.claude).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.text)
        };
        let desc_style = if is_sel {
            Style::default().fg(t.claude)
        } else {
            Style::default().fg(t.inactive)
        };
        ListItem::new(Line::from(vec![
            Span::styled(prefix, name_style),
            Span::styled(p.label, name_style),
            Span::raw("  "),
            Span::styled(p.description, desc_style),
        ]))
    }).collect();
    frame.render_widget(List::new(items), chunks[1]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(t.permission).add_modifier(Modifier::BOLD)),
            Span::styled(" navigate  ", Style::default().fg(t.subtle)),
            Span::styled("Enter", Style::default().fg(t.success).add_modifier(Modifier::BOLD)),
            Span::styled(" select", Style::default().fg(t.subtle)),
        ])),
        chunks[2],
    );
}

fn draw_key_entry(frame: &mut Frame, t: &crate::theme::Theme, provider: &ProviderOption, input: &str) {
    let full = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(t.bg)), full);

    let dialog_w = full.width.min(70);
    let dialog_h = 10u16.min(full.height);
    let x = full.x + (full.width.saturating_sub(dialog_w)) / 2;
    let y = full.y + (full.height.saturating_sub(dialog_h)) / 2;
    let dialog = Rect::new(x, y, dialog_w, dialog_h);

    frame.render_widget(Clear, dialog);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.claude))
        .title(Line::from(vec![
            Span::styled(format!(" Connect {} ", provider.label), Style::default().fg(t.claude).add_modifier(Modifier::BOLD)),
        ]))
        .style(Style::default().bg(t.bg));
    frame.render_widget(outer, dialog);

    let inner = Rect::new(dialog.x + 2, dialog.y + 2, dialog.width.saturating_sub(4), dialog.height.saturating_sub(4));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("Paste your API key and press Enter:", Style::default().fg(t.inactive)))),
        chunks[0],
    );

    let masked: String = if input.is_empty() {
        String::new()
    } else {
        let visible_end = input.len().min(6);
        format!("{}{}", &input[..visible_end], "•".repeat(input.len().saturating_sub(visible_end)))
    };
    let display = if masked.is_empty() { "▌".to_string() } else { format!("{}_", masked) };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(display, Style::default().fg(t.text).add_modifier(Modifier::BOLD)))),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(t.success).add_modifier(Modifier::BOLD)),
            Span::styled(" confirm  ", Style::default().fg(t.subtle)),
            Span::styled("Esc", Style::default().fg(t.permission).add_modifier(Modifier::BOLD)),
            Span::styled(" back", Style::default().fg(t.subtle)),
        ])),
        chunks[3],
    );
}

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
