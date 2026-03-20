use std::{collections::HashMap, path::PathBuf, time::SystemTime};

use async_trait::async_trait;
use operator_core::{ArtifactId, OperatorError, SessionId, Snapshot, SnapshotId, TargetId};
use operator_runtime::{Session, SessionEvent, SessionStore, SnapshotStore};
use tokio::sync::RwLock;

pub struct InMemorySnapshotStore {
    snapshots: RwLock<HashMap<SnapshotId, Snapshot>>,
    artifacts_root: PathBuf,
}

impl InMemorySnapshotStore {
    pub fn new() -> Self {
        Self::with_artifacts_root(std::env::temp_dir().join("operator-testkit-artifacts"))
    }

    pub fn with_artifacts_root(root: impl Into<PathBuf>) -> Self {
        Self {
            snapshots: RwLock::new(HashMap::new()),
            artifacts_root: root.into(),
        }
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SnapshotStore for InMemorySnapshotStore {
    async fn save(&self, snapshot: &Snapshot) -> Result<(), OperatorError> {
        self.snapshots
            .write()
            .await
            .insert(snapshot.id.clone(), snapshot.clone());
        Ok(())
    }

    async fn get(&self, id: &SnapshotId) -> Result<Option<Snapshot>, OperatorError> {
        Ok(self.snapshots.read().await.get(id).cloned())
    }

    async fn list(&self, target: &TargetId) -> Result<Vec<SnapshotId>, OperatorError> {
        let mut snapshots = self
            .snapshots
            .read()
            .await
            .values()
            .filter(|snapshot| &snapshot.target == target)
            .cloned()
            .collect::<Vec<_>>();

        snapshots.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });

        Ok(snapshots.into_iter().map(|snapshot| snapshot.id).collect())
    }

    async fn delete(&self, id: &SnapshotId) -> Result<(), OperatorError> {
        self.snapshots.write().await.remove(id);
        Ok(())
    }

    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError> {
        Ok(self.artifacts_root.join(&id.0))
    }

    async fn evict_expired(&self) -> Result<u32, OperatorError> {
        let now = SystemTime::now();
        let mut snapshots = self.snapshots.write().await;
        let before = snapshots.len();

        snapshots.retain(|_, snapshot| {
            snapshot
                .expires_at
                .is_none_or(|expires_at| expires_at > now)
        });

        Ok((before - snapshots.len()) as u32)
    }
}

pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<SessionId, Session>>,
    events: RwLock<HashMap<SessionId, Vec<SessionEvent>>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            events: RwLock::new(HashMap::new()),
        }
    }

    pub async fn events(&self, id: &SessionId) -> Result<Vec<SessionEvent>, OperatorError> {
        if !self.sessions.read().await.contains_key(id) {
            return Err(OperatorError::Platform(format!("session not found: {id}")));
        }

        Ok(self
            .events
            .read()
            .await
            .get(id)
            .cloned()
            .unwrap_or_default())
    }
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create(&self, session: &Session) -> Result<(), OperatorError> {
        self.sessions
            .write()
            .await
            .insert(session.id.clone(), session.clone());
        self.events
            .write()
            .await
            .entry(session.id.clone())
            .or_default();
        Ok(())
    }

    async fn append(&self, id: &SessionId, event: &SessionEvent) -> Result<(), OperatorError> {
        if !self.sessions.read().await.contains_key(id) {
            return Err(OperatorError::Platform(format!("session not found: {id}")));
        }

        self.events
            .write()
            .await
            .entry(id.clone())
            .or_default()
            .push(event.clone());
        Ok(())
    }

    async fn get(&self, id: &SessionId) -> Result<Option<Session>, OperatorError> {
        Ok(self.sessions.read().await.get(id).cloned())
    }

    async fn list(&self, limit: Option<usize>) -> Result<Vec<SessionId>, OperatorError> {
        let mut sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();

        sessions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });

        Ok(sessions
            .into_iter()
            .take(limit.unwrap_or(100))
            .map(|session| session.id)
            .collect())
    }
}
