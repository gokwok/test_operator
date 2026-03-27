use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use operator_core::{OperatorError, PlatformDriver};

use crate::{
    tools, ArtifactStore, EventSink, NullEventSink, NullSessionStore, Runtime, RuntimeConfig,
    RuntimeCore, SessionStore, SnapshotStore, TargetResolver, ToolRegistry,
};
use crate::{PlatformDriverFactory, PlatformRegistry};

pub struct RuntimeBuilder {
    config: RuntimeConfig,
    platform_registry: PlatformRegistry,
    artifacts: Option<Arc<dyn ArtifactStore>>,
    snapshots: Option<Arc<dyn SnapshotStore>>,
    sessions: Arc<dyn SessionStore>,
    event_sink: Arc<dyn EventSink>,
}

impl RuntimeBuilder {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            platform_registry: PlatformRegistry::new(),
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
        self.platform_registry
            .register_factory(Arc::new(StaticDriverFactory::new(driver)));
        self
    }

    pub fn register_drivers<I>(mut self, drivers: I) -> Self
    where
        I: IntoIterator<Item = Arc<dyn PlatformDriver>>,
    {
        for driver in drivers {
            self = self.register_driver(driver);
        }

        self
    }

    pub fn register_factory(mut self, factory: Arc<dyn PlatformDriverFactory>) -> Self {
        self.platform_registry.register_factory(factory);
        self
    }

    pub fn platform_registry(mut self, registry: PlatformRegistry) -> Self {
        self.platform_registry.extend(registry);
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
            platform_registry: self.platform_registry,
            artifacts,
            snapshots,
            sessions: self.sessions,
            event_sink: self.event_sink,
            config: self.config,
            driver_cache: std::sync::Mutex::new(HashMap::new()),
        };

        let core = Arc::new(core);
        let mut tools = ToolRegistry::new(core.clone());
        tools.register_all(tools::registrations())?;

        Ok(Runtime { core, tools })
    }
}

struct StaticDriverFactory {
    driver: Arc<dyn PlatformDriver>,
}

impl StaticDriverFactory {
    fn new(driver: Arc<dyn PlatformDriver>) -> Self {
        Self { driver }
    }
}

impl PlatformDriverFactory for StaticDriverFactory {
    fn driver_id(&self) -> &str {
        self.driver.driver_id()
    }

    fn build(
        &self,
        target: &operator_core::TargetDescriptor,
    ) -> Result<Arc<dyn PlatformDriver>, OperatorError> {
        if target.platform != self.driver.platform_id() {
            return Err(OperatorError::Platform(format!(
                "target {} resolved to platform {}, but registered driver {} serves {}",
                target.id,
                target.platform,
                self.driver.driver_id(),
                self.driver.platform_id()
            )));
        }

        if !target.driver_config.is_empty() {
            return Err(OperatorError::Platform(format!(
                "driver {} does not accept target-level driver_config",
                self.driver.driver_id()
            )));
        }

        Ok(Arc::clone(&self.driver))
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
