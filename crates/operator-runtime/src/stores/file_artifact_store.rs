use std::path::{Path, PathBuf};

use async_trait::async_trait;
use operator_core::{ArtifactId, OperatorError};
use tokio::fs;

use crate::stores::ArtifactStore;

pub struct FileArtifactStore {
    root: PathBuf,
}

impl FileArtifactStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    fn artifact_path(&self, id: &ArtifactId) -> PathBuf {
        self.artifacts_dir().join(&id.0)
    }

    async fn ensure_dir(&self) -> Result<(), OperatorError> {
        fs::create_dir_all(self.artifacts_dir()).await?;
        Ok(())
    }
}

#[async_trait]
impl ArtifactStore for FileArtifactStore {
    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError> {
        self.ensure_dir().await?;
        Ok(self.artifact_path(id))
    }
}
