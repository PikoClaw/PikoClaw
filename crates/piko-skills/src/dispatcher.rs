use crate::registry::SkillRegistry;

#[derive(Debug, Clone)]
pub enum DispatchResult {
    Skill {
        name: String,
        rendered_prompt: Option<String>,
        args: Vec<String>,
    },
    BuiltIn {
        name: String,
        args: Vec<String>,
    },
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
            "theme".to_string(),
            "exit".to_string(),
            "quit".to_string(),
            "plan".to_string(),
            "cost".to_string(),
            "connect".to_string(),
        ];
        Self {
            registry,
            built_ins,
        }
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

    pub fn slash_commands(&self) -> Vec<(String, String)> {
        let mut commands: Vec<(String, String)> = self
            .registry
            .list()
            .into_iter()
            .map(|skill| (skill.name.clone(), skill.description.clone()))
            .collect();
        commands.sort_by(|a, b| a.0.cmp(&b.0));
        commands.dedup_by(|a, b| a.0 == b.0);
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::SkillRegistry;

    fn make_dispatcher() -> SkillDispatcher {
        SkillDispatcher::new(SkillRegistry::new())
    }

    #[test]
    fn plan_is_builtin() {
        let d = make_dispatcher();
        let result = d.dispatch("/plan");
        assert!(matches!(result, DispatchResult::BuiltIn { ref name, .. } if name == "plan"));
    }

    #[test]
    fn cost_is_builtin() {
        let d = make_dispatcher();
        let result = d.dispatch("/cost");
        assert!(matches!(result, DispatchResult::BuiltIn { ref name, .. } if name == "cost"));
    }

    #[test]
    fn connect_is_builtin() {
        let d = make_dispatcher();
        let result = d.dispatch("/connect");
        assert!(matches!(result, DispatchResult::BuiltIn { ref name, .. } if name == "connect"));
    }

    #[test]
    fn non_command_returns_not_a_command() {
        let d = make_dispatcher();
        let result = d.dispatch("hello world");
        assert!(matches!(result, DispatchResult::NotACommand));
    }

    #[test]
    fn unknown_slash_command_returns_not_a_command() {
        let d = make_dispatcher();
        let result = d.dispatch("/nonexistent");
        assert!(matches!(result, DispatchResult::NotACommand));
    }

    #[test]
    fn help_is_builtin() {
        let d = make_dispatcher();
        let result = d.dispatch("/help");
        assert!(matches!(result, DispatchResult::BuiltIn { ref name, .. } if name == "help"));
    }

    #[test]
    fn clear_is_builtin() {
        let d = make_dispatcher();
        let result = d.dispatch("/clear");
        assert!(matches!(result, DispatchResult::BuiltIn { ref name, .. } if name == "clear"));
    }
}
