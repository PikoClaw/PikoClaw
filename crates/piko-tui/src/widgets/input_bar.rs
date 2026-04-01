use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub struct InputBar;

impl InputBar {
    pub fn render(frame: &mut Frame, area: Rect, input: &str, cursor_pos: usize) {
        let display = format!("> {}{}", &input[..cursor_pos], &input[cursor_pos..]);
        let widget = Paragraph::new(display)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(widget, area);
    }
}
