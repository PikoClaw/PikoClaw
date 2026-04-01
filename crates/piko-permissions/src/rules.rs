use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleDecision {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub tool: String,
    pub pattern: String,
    pub decision: RuleDecision,
}

#[derive(Debug)]
pub struct CompiledRules {
    rules: Vec<(Rule, GlobSet)>,
}

impl CompiledRules {
    pub fn compile(rules: &[Rule]) -> Result<Self, globset::Error> {
        let mut compiled = Vec::new();
        for rule in rules {
            let mut builder = GlobSetBuilder::new();
            builder.add(Glob::new(&rule.pattern)?);
            let glob_set = builder.build()?;
            compiled.push((rule.clone(), glob_set));
        }
        Ok(Self { rules: compiled })
    }

    pub fn check(&self, tool: &str, input_str: &str) -> Option<&RuleDecision> {
        for (rule, glob_set) in &self.rules {
            if (rule.tool == tool || rule.tool == "*") && glob_set.is_match(input_str) {
                return Some(&rule.decision);
            }
        }
        None
    }
}
