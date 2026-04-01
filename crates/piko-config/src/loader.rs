use crate::config::PikoConfig;
use crate::env;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = env::pikoclaw_config_path() {
        return Some(PathBuf::from(path));
    }
    ProjectDirs::from("dev", "pikoclaw", "pikoclaw")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn load_config() -> Result<PikoConfig> {
    let mut config = PikoConfig::default();

    if let Some(path) = config_path() {
        if path.exists() {
            config = load_from_file(&path)
                .with_context(|| format!("failed to load config from {}", path.display()))?;
        }
    }

    if let Some(api_key) = env::anthropic_api_key() {
        config.api.api_key = Some(api_key);
    }

    if let Some(base_url) = env::anthropic_base_url() {
        config.api.base_url = base_url;
    }

    if let Some(model) = env::anthropic_model() {
        config.api.model = model.into();
    }

    Ok(config)
}

fn load_from_file(path: &Path) -> Result<PikoConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: PikoConfig = toml::from_str(&content)?;
    Ok(config)
}
