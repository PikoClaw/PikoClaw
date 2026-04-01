use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

pub struct ChatPane;

impl ChatPane {
    pub fn render(frame: &mut Frame, area: Rect, items: Vec<ListItem>) {
        let block = Block::default().borders(Borders::ALL).title(" Chat ");
        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}
