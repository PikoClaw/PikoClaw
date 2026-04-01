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
}
