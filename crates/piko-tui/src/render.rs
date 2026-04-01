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

    let status_text = match app.state {
        AppState::WaitingForAgent => " thinking... ",
        AppState::Running => " ready ",
        AppState::Exiting => " exiting... ",
    };

    let status = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, chunks[1]);

    let cursor_display = format!(
        "{}{}",
        &app.input[..app.cursor_pos],
        if app.state == AppState::Running {
            "█"
        } else {
            ""
        }
    );
    let input_widget = Paragraph::new(format!(
        "> {}{}",
        cursor_display,
        &app.input[app.cursor_pos..]
    ))
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: false });
    frame.render_widget(input_widget, chunks[2]);
}
