use crate::index::SessionIndex;
use crate::session::{Session, SessionInfo};
use crate::store::SessionStore;
use anyhow::{Context, Result};
use async_trait::async_trait;
use directories::ProjectDirs;
use std::path::PathBuf;
use tokio::fs;
use tracing::warn;

pub struct FilesystemSessionStore {
    base_dir: PathBuf,
}

impl FilesystemSessionStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn default_path() -> PathBuf {
        ProjectDirs::from("dev", "pikoclaw", "pikoclaw")
            .map(|dirs| dirs.data_dir().join("sessions"))
            .unwrap_or_else(|| PathBuf::from(".pikoclaw/sessions"))
    }

    pub fn with_default_path() -> Self {
        Self::new(Self::default_path())
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", id))
    }

    fn index_path(&self) -> PathBuf {
        self.base_dir.join("index.json")
    }
}

#[async_trait]
impl SessionStore for FilesystemSessionStore {
    async fn save(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.base_dir).await
            .context("failed to create sessions directory")?;

        let path = self.session_path(&session.id);
        let tmp = path.with_extension("tmp");
        let content = serde_json::to_string_pretty(session)?;
        fs::write(&tmp, content).await?;
        fs::rename(&tmp, &path).await?;

        let index_path = self.index_path();
        let mut index = SessionIndex::load(&index_path).unwrap_or_default();
        index.set_latest(&session.cwd, &session.id);
        tokio::task::spawn_blocking(move || index.save(&index_path)).await??;

        Ok(())
    }

    async fn load(&self, id: &str) -> Result<Option<Session>> {
        let path = self.session_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path).await?;
        let session = serde_json::from_str(&content)?;
        Ok(Some(session))
    }

    async fn list(&self) -> Result<Vec<SessionInfo>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&self.base_dir).await?;
        let mut infos = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if path.file_stem().and_then(|s| s.to_str()) == Some("index") {
                continue;
            }
            match fs::read_to_string(&path).await {
                Ok(content) => match serde_json::from_str::<Session>(&content) {
                    Ok(session) => infos.push(SessionInfo::from(&session)),
                    Err(e) => warn!("failed to parse session {}: {}", path.display(), e),
                },
                Err(e) => warn!("failed to read session {}: {}", path.display(), e),
            }
        }

        infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(infos)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn latest_for_cwd(&self, cwd: &str) -> Result<Option<Session>> {
        let index_path = self.index_path();
        let index = SessionIndex::load(&index_path).unwrap_or_default();
        if let Some(id) = index.get_latest(cwd) {
            return self.load(id).await;
        }
        Ok(None)
    }
}
