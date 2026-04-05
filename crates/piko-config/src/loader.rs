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

    // ANTHROPIC_AUTH_TOKEN enables Bearer-token auth for third-party providers (e.g. OpenRouter).
    if let Some(auth_token) = env::anthropic_auth_token() {
        config.api.auth_token = Some(auth_token);
    }

    if let Some(base_url) = env::anthropic_base_url() {
        config.api.base_url = base_url;
    }

    if let Some(provider) = env::pikoclaw_provider() {
        config.api.provider = Some(provider);
    }

    // ANTHROPIC_DEFAULT_SONNET_MODEL mirrors claude-code's model-slot override.
    if let Some(model) = env::anthropic_default_sonnet_model().or_else(env::anthropic_model) {
        config.api.model = model.into();
    }

    Ok(config)
}

fn load_from_file(path: &Path) -> Result<PikoConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: PikoConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Persist `config` back to the user's config file.
/// Creates parent directories if they don't exist yet.
pub fn save_config(config: &PikoConfig) -> Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config path"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}
