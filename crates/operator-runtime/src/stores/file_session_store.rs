use std::path::{Path, PathBuf};

use async_trait::async_trait;
use operator_core::{OperatorError, SessionId};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

use crate::{Session, SessionEvent, SessionStore};

pub struct FileSessionStore {
    root: PathBuf,
}

impl FileSessionStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    fn session_path(&self, id: &SessionId) -> PathBuf {
        self.sessions_dir().join(format!("{}.json", id.0))
    }

    fn session_log_path(&self, id: &SessionId) -> PathBuf {
        self.sessions_dir().join(format!("{}.jsonl", id.0))
    }

    async fn ensure_dir(&self) -> Result<(), OperatorError> {
        fs::create_dir_all(self.sessions_dir()).await?;
        Ok(())
    }

    async fn load_sessions(&self) -> Result<Vec<Session>, OperatorError> {
        self.ensure_dir().await?;

        let mut entries = fs::read_dir(self.sessions_dir()).await?;
        let mut sessions = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if entry.file_type().await?.is_file()
                && path.extension().is_some_and(|ext| ext == "json")
            {
                let bytes = fs::read(path).await?;
                let session = serde_json::from_slice::<Session>(&bytes)?;
                sessions.push(session);
            }
        }

        Ok(sessions)
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn create(&self, session: &Session) -> Result<(), OperatorError> {
        self.ensure_dir().await?;

        let path = self.session_path(&session.id);
        let bytes = serde_json::to_vec_pretty(session)?;
        fs::write(path, bytes).await?;
        Ok(())
    }

    async fn append(&self, id: &SessionId, event: &SessionEvent) -> Result<(), OperatorError> {
        self.ensure_dir().await?;

        if !self.session_path(id).exists() {
            return Err(OperatorError::Platform(format!("session not found: {id}")));
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.session_log_path(id))
            .await?;
        let mut line = serde_json::to_vec(event)?;
        line.push(b'\n');
        file.write_all(&line).await?;
        file.flush().await?;

        Ok(())
    }

    async fn get(&self, id: &SessionId) -> Result<Option<Session>, OperatorError> {
        let path = self.session_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path).await?;
        let session = serde_json::from_slice::<Session>(&bytes)?;
        Ok(Some(session))
    }

    async fn list(&self, limit: Option<usize>) -> Result<Vec<SessionId>, OperatorError> {
        let mut sessions = self.load_sessions().await?;
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let limit = limit.unwrap_or(100);
        Ok(sessions
            .into_iter()
            .take(limit)
            .map(|session| session.id)
            .collect())
    }
}
