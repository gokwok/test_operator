use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::SystemTime,
};

use async_trait::async_trait;
use operator_core::{ArtifactId, OperatorError, Snapshot, SnapshotId, TargetId};
use tokio::fs;

use crate::{RuntimeConfig, SnapshotStore};

pub struct FileSnapshotStore {
    root: PathBuf,
    config: RuntimeConfig,
    save_count: AtomicU32,
}

impl FileSnapshotStore {
    pub fn new(root: impl AsRef<Path>, config: RuntimeConfig) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            config,
            save_count: AtomicU32::new(0),
        }
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }

    fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    fn snapshot_path(&self, id: &SnapshotId) -> PathBuf {
        self.snapshots_dir().join(format!("{}.json", id.0))
    }

    fn artifact_path(&self, id: &ArtifactId) -> PathBuf {
        self.artifacts_dir().join(&id.0)
    }

    async fn ensure_dirs(&self) -> Result<(), OperatorError> {
        fs::create_dir_all(self.snapshots_dir()).await?;
        fs::create_dir_all(self.artifacts_dir()).await?;
        Ok(())
    }

    async fn load_snapshots(&self) -> Result<Vec<Snapshot>, OperatorError> {
        self.ensure_dirs().await?;

        let mut entries = fs::read_dir(self.snapshots_dir()).await?;
        let mut snapshots = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                let bytes = fs::read(entry.path()).await?;
                let snapshot = serde_json::from_slice::<Snapshot>(&bytes)?;
                snapshots.push(snapshot);
            }
        }

        Ok(snapshots)
    }

    async fn delete_snapshot_files(&self, snapshot: &Snapshot) -> Result<(), OperatorError> {
        remove_if_exists(self.snapshot_path(&snapshot.id)).await?;

        if let Some(artifact_id) = &snapshot.image_artifact {
            remove_if_exists(self.artifact_path(artifact_id)).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl SnapshotStore for FileSnapshotStore {
    async fn save(&self, snapshot: &Snapshot) -> Result<(), OperatorError> {
        self.ensure_dirs().await?;

        let path = self.snapshot_path(&snapshot.id);
        let bytes = serde_json::to_vec_pretty(snapshot)?;
        fs::write(path, bytes).await?;

        let save_count = self.save_count.fetch_add(1, Ordering::Relaxed) + 1;
        if self.config.snapshot_evict_interval > 0
            && save_count % self.config.snapshot_evict_interval == 0
        {
            self.evict_expired().await?;
        }

        Ok(())
    }

    async fn get(&self, id: &SnapshotId) -> Result<Option<Snapshot>, OperatorError> {
        let path = self.snapshot_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path).await?;
        let snapshot = serde_json::from_slice::<Snapshot>(&bytes)?;
        Ok(Some(snapshot))
    }

    async fn list(&self, target: &TargetId) -> Result<Vec<SnapshotId>, OperatorError> {
        let mut snapshots = self.load_snapshots().await?;
        snapshots.retain(|snapshot| &snapshot.target == target);
        snapshots.sort_by(|left, right| left.created_at.cmp(&right.created_at));

        Ok(snapshots.into_iter().map(|snapshot| snapshot.id).collect())
    }

    async fn delete(&self, id: &SnapshotId) -> Result<(), OperatorError> {
        if let Some(snapshot) = self.get(id).await? {
            self.delete_snapshot_files(&snapshot).await?;
        }

        Ok(())
    }

    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError> {
        self.ensure_dirs().await?;
        Ok(self.artifact_path(id))
    }

    async fn evict_expired(&self) -> Result<u32, OperatorError> {
        let now = SystemTime::now();
        let mut snapshots = self.load_snapshots().await?;
        let mut removed = 0;

        let mut expired = Vec::new();
        let mut active = Vec::new();
        for snapshot in snapshots.drain(..) {
            if snapshot
                .expires_at
                .is_some_and(|expires_at| expires_at <= now)
            {
                expired.push(snapshot);
            } else {
                active.push(snapshot);
            }
        }

        for snapshot in expired {
            self.delete_snapshot_files(&snapshot).await?;
            removed += 1;
        }

        if active.len() > self.config.max_snapshots {
            active.sort_by(|left, right| left.created_at.cmp(&right.created_at));
            let overflow = active.len() - self.config.max_snapshots;
            for snapshot in active.into_iter().take(overflow) {
                self.delete_snapshot_files(&snapshot).await?;
                removed += 1;
            }
        }

        Ok(removed)
    }
}

async fn remove_if_exists(path: PathBuf) -> Result<(), OperatorError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
