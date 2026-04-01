use crate::session::{Session, SessionInfo};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, session: &Session) -> Result<()>;
    async fn load(&self, id: &str) -> Result<Option<Session>>;
    async fn list(&self) -> Result<Vec<SessionInfo>>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn latest_for_cwd(&self, cwd: &str) -> Result<Option<Session>>;
}

#[async_trait]
impl<T: SessionStore + ?Sized> SessionStore for std::sync::Arc<T> {
    async fn save(&self, session: &Session) -> Result<()> {
        (**self).save(session).await
    }
    async fn load(&self, id: &str) -> Result<Option<Session>> {
        (**self).load(id).await
    }
    async fn list(&self) -> Result<Vec<SessionInfo>> {
        (**self).list().await
    }
    async fn delete(&self, id: &str) -> Result<()> {
        (**self).delete(id).await
    }
    async fn latest_for_cwd(&self, cwd: &str) -> Result<Option<Session>> {
        (**self).latest_for_cwd(cwd).await
    }
}
