use crate::app::{App, AppState, MessageRole};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            let (prefix, style) = match msg.role {
                MessageRole::User => (
                    "you> ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Assistant => ("ai>  ", Style::default().fg(Color::Green)),
                MessageRole::System => (
                    "sys> ",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            };

            let lines: Vec<Line> = msg
                .content
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        Line::from(vec![Span::styled(prefix, style), Span::styled(line, style)])
                    } else {
                        Line::from(vec![Span::raw("     "), Span::styled(line, style)])
                    }
                })
                .collect();

            ListItem::new(Text::from(lines))
        })
        .collect();

    let messages_block = Block::default().borders(Borders::ALL).title(" PikoClaw ");
    let list = List::new(items).block(messages_block);
    frame.render_widget(list, chunks[0]);

    let token_info = if app.total_input_tokens > 0 || app.total_output_tokens > 0 {
        format!(" ↑{}  ↓{}", app.total_input_tokens, app.total_output_tokens)
    } else {
        String::new()
    };
    let status_text = match app.state {
        AppState::WaitingForAgent => format!(" thinking...{}", token_info),
        AppState::Running => format!(" ready{}", token_info),
        AppState::AskingPermission => format!(" permission required{}", token_info),
        AppState::AskingQuestion => format!(" question{}", token_info),
        AppState::Exiting => format!(" exiting...{}", token_info),
    };
    let status_style =
        if app.state == AppState::AskingPermission || app.state == AppState::AskingQuestion {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
    let status = Paragraph::new(status_text).style(status_style);
    frame.render_widget(status, chunks[1]);

    let input_content = if app.state == AppState::AskingPermission {
        if let Some(ref prompt) = app.pending_permission {
            let desc = &prompt.request.description;
            format!(
                "Allow [{}]? (y)es / (n)o / (a)lways / (d)eny-always\n{}",
                prompt.request.tool_name,
                &desc[..desc.len().min(120)]
            )
        } else {
            "Allow? (y/n/a/d)".to_string()
        }
    } else if app.state == AppState::AskingQuestion {
        if let Some(ref prompt) = app.pending_question {
            let opts: String = prompt
                .options
                .iter()
                .enumerate()
                .map(|(i, o)| format!("  [{}] {}", i + 1, o))
                .collect::<Vec<_>>()
                .join("\n");
            format!("{}\n{}\nPress number to select:", prompt.question, opts)
        } else {
            "Question pending...".to_string()
        }
    } else {
        let cursor_display = format!(
            "{}{}",
            &app.input[..app.cursor_pos],
            if app.state == AppState::Running {
                "█"
            } else {
                ""
            }
        );
        format!("> {}{}", cursor_display, &app.input[app.cursor_pos..])
    };

    let input_widget = Paragraph::new(input_content)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(input_widget, chunks[2]);
}
