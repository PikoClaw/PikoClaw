pub mod app;
pub mod events;
pub mod highlight;
pub mod history;
pub mod onboarding;
pub mod render;
pub mod theme;
pub mod tui_output;
pub mod tui_permissions;
pub mod widgets;

pub use app::{App, AppState};
pub use history::InputHistory;
