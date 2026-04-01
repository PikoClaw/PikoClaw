use crate::skill::Skill;
use std::collections::HashMap;

pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn list(&self) -> Vec<&Skill> {
        let mut skills: Vec<&Skill> = self.skills.values().collect();
        skills.sort_by_key(|s| &s.name);
        skills
    }

    pub fn with_built_ins() -> Self {
        use crate::built_ins::register_all;
        let mut registry = Self::new();
        register_all(&mut registry);
        registry
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
