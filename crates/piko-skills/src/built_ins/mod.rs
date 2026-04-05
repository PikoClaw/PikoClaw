use crate::registry::SkillRegistry;
use crate::skill::{Skill, SkillSource};

pub fn register_all(registry: &mut SkillRegistry) {
    registry.register(Skill {
        name: "help".to_string(),
        description: "Show available commands and skills".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "clear".to_string(),
        description: "Clear conversation history".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "model".to_string(),
        description: "Switch the active model (e.g. /model sonnet)".to_string(),
        args: vec!["name".to_string()],
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "compact".to_string(),
        description: "Summarize conversation history to reduce token usage".to_string(),
        args: Vec::new(),
        prompt_template: "Please create a comprehensive summary of our conversation so far, capturing all important context, decisions made, code written, and current state. This summary will replace the full history.".to_string(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "theme".to_string(),
        description: "Switch UI theme. /theme to cycle, /theme <name> to set (dark, light, dark-daltonized, light-daltonized, dark-ansi, light-ansi)".to_string(),
        args: vec!["name".to_string()],
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "connect".to_string(),
        description: "Connect a provider and save its API key".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "cost".to_string(),
        description: "Show the current session cost summary".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "plan".to_string(),
        description: "Toggle plan mode for read-only tool execution".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "exit".to_string(),
        description: "Exit the application".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });

    registry.register(Skill {
        name: "quit".to_string(),
        description: "Exit the application".to_string(),
        args: Vec::new(),
        prompt_template: String::new(),
        source: SkillSource::BuiltIn,
    });
}
