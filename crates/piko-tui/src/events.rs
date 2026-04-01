use crossterm::event::KeyEvent;
use piko_agent::AgentEvent;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Agent(AgentEvent),
    Tick,
    Quit,
}
