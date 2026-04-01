use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

pub struct StatusBar;

impl StatusBar {
    pub fn render(frame: &mut Frame, area: Rect, model: &str, status: &str) {
        let text = format!(" {} | {} ", model, status);
        let widget = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(widget, area);
    }
}
