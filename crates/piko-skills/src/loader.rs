use crate::registry::SkillRegistry;
use crate::skill::{Skill, SkillSource};
use anyhow::Result;
use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use tracing::warn;

pub fn skills_dir() -> Option<PathBuf> {
    ProjectDirs::from("dev", "pikoclaw", "pikoclaw")
        .map(|dirs| dirs.config_dir().join("skills"))
}

pub fn load_user_skills(registry: &mut SkillRegistry) -> Result<()> {
    let dir = match skills_dir() {
        Some(d) => d,
        None => return Ok(()),
    };

    if !dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        match load_skill_file(&path) {
            Ok(skill) => registry.register(skill),
            Err(e) => warn!("failed to load skill {}: {}", path.display(), e),
        }
    }

    Ok(())
}

fn load_skill_file(path: &Path) -> Result<Skill> {
    let content = std::fs::read_to_string(path)?;

    let (frontmatter, body) = if content.starts_with("---\n") {
        let end = content[4..].find("\n---").map(|i| i + 4);
        match end {
            Some(end_idx) => {
                let fm = &content[4..end_idx];
                let body = content[end_idx + 4..].trim().to_string();
                (fm.to_string(), body)
            }
            None => (String::new(), content.clone()),
        }
    } else {
        (String::new(), content.clone())
    };

    #[derive(serde::Deserialize)]
    struct FrontMatter {
        name: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        args: Vec<String>,
    }

    let fm: FrontMatter = toml::from_str(&frontmatter)
        .map_err(|e| anyhow::anyhow!("invalid frontmatter: {}", e))?;

    Ok(Skill {
        name: fm.name,
        description: fm.description,
        args: fm.args,
        prompt_template: body,
        source: SkillSource::User,
    })
}
