mod file_session_store;
mod file_snapshot_store;
mod null_session_store;

use std::path::PathBuf;

use async_trait::async_trait;
use operator_core::{ArtifactId, OperatorError, SessionId, Snapshot, SnapshotId, TargetId};

use crate::{Session, SessionEvent};

pub use file_session_store::FileSessionStore;
pub use file_snapshot_store::FileSnapshotStore;
pub use null_session_store::NullSessionStore;

#[async_trait]
pub trait SnapshotStore: Send + Sync {
    async fn save(&self, snapshot: &Snapshot) -> Result<(), OperatorError>;
    async fn get(&self, id: &SnapshotId) -> Result<Option<Snapshot>, OperatorError>;
    async fn list(&self, target: &TargetId) -> Result<Vec<SnapshotId>, OperatorError>;
    async fn delete(&self, id: &SnapshotId) -> Result<(), OperatorError>;
    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError>;
    async fn evict_expired(&self) -> Result<u32, OperatorError>;
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: &Session) -> Result<(), OperatorError>;
    async fn append(&self, id: &SessionId, event: &SessionEvent) -> Result<(), OperatorError>;
    async fn get(&self, id: &SessionId) -> Result<Option<Session>, OperatorError>;
    async fn list(&self, limit: Option<usize>) -> Result<Vec<SessionId>, OperatorError>;
}
