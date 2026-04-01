use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionIndex {
    pub latest_by_cwd: HashMap<String, String>,
}

impl SessionIndex {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let index = serde_json::from_str(&content)?;
        Ok(index)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, content)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn set_latest(&mut self, cwd: &str, session_id: &str) {
        self.latest_by_cwd
            .insert(cwd.to_string(), session_id.to_string());
    }

    pub fn get_latest(&self, cwd: &str) -> Option<&str> {
        self.latest_by_cwd.get(cwd).map(|s| s.as_str())
    }
}
