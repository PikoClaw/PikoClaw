use crate::rules::{CompiledRules, Rule, RuleDecision};
use piko_config::config::PermissionMode;
use std::collections::HashMap;

pub struct PermissionPolicy {
    tool_modes: HashMap<String, PermissionMode>,
    default_mode: PermissionMode,
    compiled_rules: CompiledRules,
}

impl PermissionPolicy {
    pub fn new(
        tool_modes: HashMap<String, PermissionMode>,
        default_mode: PermissionMode,
        rules: &[Rule],
    ) -> Result<Self, globset::Error> {
        let compiled_rules = CompiledRules::compile(rules)?;
        Ok(Self {
            tool_modes,
            default_mode,
            compiled_rules,
        })
    }

    pub fn from_config(config: &piko_config::config::PermissionsConfig) -> Self {
        let mut tool_modes = HashMap::new();
        tool_modes.insert("bash".to_string(), config.bash.clone());
        tool_modes.insert("file_write".to_string(), config.file_write.clone());
        tool_modes.insert("file_read".to_string(), config.file_read.clone());
        tool_modes.insert("web_fetch".to_string(), config.web_fetch.clone());

        let rules: Vec<Rule> = config
            .rules
            .iter()
            .map(|r| Rule {
                tool: r.tool.clone(),
                pattern: r.pattern.clone(),
                decision: match r.decision {
                    PermissionMode::Allow => RuleDecision::Allow,
                    PermissionMode::Deny => RuleDecision::Deny,
                    PermissionMode::Ask => RuleDecision::Ask,
                },
            })
            .collect();

        let compiled_rules = CompiledRules::compile(&rules).unwrap_or_else(|_| {
            CompiledRules::compile(&[]).unwrap()
        });

        Self {
            tool_modes,
            default_mode: config.default_mode.clone(),
            compiled_rules,
        }
    }

    pub fn lookup(&self, tool: &str, input_str: &str) -> &PermissionMode {
        if let Some(rule_decision) = self.compiled_rules.check(tool, input_str) {
            return match rule_decision {
                RuleDecision::Allow => &PermissionMode::Allow,
                RuleDecision::Deny => &PermissionMode::Deny,
                RuleDecision::Ask => &PermissionMode::Ask,
            };
        }

        self.tool_modes.get(tool).unwrap_or(&self.default_mode)
    }
}
