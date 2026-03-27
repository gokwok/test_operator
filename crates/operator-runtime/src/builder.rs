use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use operator_core::{OperatorError, PlatformDriver};

use crate::{
    tools, ArtifactStore, EventSink, NullEventSink, NullSessionStore, Runtime, RuntimeConfig,
    RuntimeCore, SessionStore, SnapshotStore, TargetResolver, ToolRegistry,
};

pub struct RuntimeBuilder {
    config: RuntimeConfig,
    drivers: HashMap<String, Arc<dyn PlatformDriver>>,
    artifacts: Option<Arc<dyn ArtifactStore>>,
    snapshots: Option<Arc<dyn SnapshotStore>>,
    sessions: Arc<dyn SessionStore>,
    event_sink: Arc<dyn EventSink>,
}

impl RuntimeBuilder {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            drivers: HashMap::new(),
            artifacts: None,
            snapshots: None,
            sessions: Arc::new(NullSessionStore),
            event_sink: Arc::new(NullEventSink),
        }
    }

    pub fn artifact_store(mut self, store: Arc<dyn ArtifactStore>) -> Self {
        self.artifacts = Some(store);
        self
    }

    pub fn snapshot_store(mut self, store: Arc<dyn SnapshotStore>) -> Self {
        self.snapshots = Some(store);
        self
    }

    pub fn session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.sessions = store;
        self
    }

    pub fn event_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.event_sink = sink;
        self
    }

    pub fn register_driver(mut self, driver: Arc<dyn PlatformDriver>) -> Self {
        self.drivers.insert(driver.driver_id().to_string(), driver);
        self
    }

    pub async fn build(self) -> Result<Runtime, OperatorError> {
        let snapshots = self.snapshots.ok_or_else(|| {
            OperatorError::Platform("runtime builder requires a snapshot store".into())
        })?;
        snapshots.evict_expired().await?;
        let artifacts = self.artifacts.unwrap_or_else(|| {
            Arc::new(SnapshotArtifactStore::new(Arc::clone(&snapshots))) as Arc<dyn ArtifactStore>
        });

        let default_target = self.config.default_target.clone();
        let named_targets = self.config.targets.clone();
        let core = RuntimeCore {
            resolver: TargetResolver::new(default_target, named_targets),
            drivers: self.drivers,
            artifacts,
            snapshots,
            sessions: self.sessions,
            event_sink: self.event_sink,
            config: self.config,
        };

        let core = Arc::new(core);
        let mut tools = ToolRegistry::new(core.clone());
        tools.register_all(tools::registrations())?;

        Ok(Runtime { core, tools })
    }
}

struct SnapshotArtifactStore {
    snapshots: Arc<dyn SnapshotStore>,
}

impl SnapshotArtifactStore {
    fn new(snapshots: Arc<dyn SnapshotStore>) -> Self {
        Self { snapshots }
    }
}

#[async_trait]
impl ArtifactStore for SnapshotArtifactStore {
    async fn resolve_artifact(
        &self,
        id: &operator_core::ArtifactId,
    ) -> Result<std::path::PathBuf, OperatorError> {
        self.snapshots.resolve_artifact(id).await
    }
}
