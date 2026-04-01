use crate::registry::SkillRegistry;

#[derive(Debug, Clone)]
pub enum DispatchResult {
    Skill { name: String, rendered_prompt: Option<String>, args: Vec<String> },
    BuiltIn { name: String, args: Vec<String> },
    NotACommand,
}

pub struct SkillDispatcher {
    registry: SkillRegistry,
    built_ins: Vec<String>,
}

impl SkillDispatcher {
    pub fn new(registry: SkillRegistry) -> Self {
        let built_ins = vec![
            "help".to_string(),
            "clear".to_string(),
            "model".to_string(),
            "compact".to_string(),
            "exit".to_string(),
            "quit".to_string(),
        ];
        Self { registry, built_ins }
    }

    pub fn dispatch(&self, input: &str) -> DispatchResult {
        let input = input.trim();
        if !input.starts_with('/') {
            return DispatchResult::NotACommand;
        }

        let without_slash = &input[1..];
        let mut parts = without_slash.splitn(2, ' ');
        let name = parts.next().unwrap_or("").to_lowercase();
        let args_str = parts.next().unwrap_or("").trim();
        let args: Vec<String> = if args_str.is_empty() {
            Vec::new()
        } else {
            args_str.split_whitespace().map(|s| s.to_string()).collect()
        };

        if self.built_ins.contains(&name) {
            return DispatchResult::BuiltIn { name, args };
        }

        if let Some(skill) = self.registry.get(&name) {
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let rendered = if !skill.prompt_template.is_empty() {
                Some(skill.render(&arg_refs))
            } else {
                None
            };
            return DispatchResult::Skill {
                name,
                rendered_prompt: rendered,
                args,
            };
        }

        DispatchResult::NotACommand
    }
}
