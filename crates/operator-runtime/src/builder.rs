use std::{collections::HashMap, sync::Arc};

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor, TargetId};

use crate::{
    EventSink, NullEventSink, NullSessionStore, RuntimeConfig, SessionStore, SnapshotStore,
    TargetResolver,
};

pub struct RuntimeCore {
    resolver: TargetResolver,
    drivers: HashMap<String, Arc<dyn PlatformDriver>>,
    snapshots: Arc<dyn SnapshotStore>,
    sessions: Arc<dyn SessionStore>,
    event_sink: Arc<dyn EventSink>,
    config: RuntimeConfig,
}

impl RuntimeCore {
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn snapshots(&self) -> Arc<dyn SnapshotStore> {
        Arc::clone(&self.snapshots)
    }

    pub fn sessions(&self) -> Arc<dyn SessionStore> {
        Arc::clone(&self.sessions)
    }

    pub fn event_sink(&self) -> Arc<dyn EventSink> {
        Arc::clone(&self.event_sink)
    }

    pub fn resolve_target(
        &self,
        target: Option<&TargetId>,
    ) -> Result<TargetDescriptor, OperatorError> {
        self.resolver.resolve(target)
    }

    pub fn resolve_driver(
        &self,
        target: &TargetId,
    ) -> Result<(TargetDescriptor, Arc<dyn PlatformDriver>), OperatorError> {
        let descriptor = self.resolve_target(Some(target))?;
        let driver = self
            .drivers
            .get(&descriptor.platform)
            .cloned()
            .ok_or_else(|| OperatorError::TargetNotFound(target.to_string()))?;

        Ok((descriptor, driver))
    }
}

#[derive(Clone)]
pub struct Runtime {
    core: Arc<RuntimeCore>,
}

impl Runtime {
    pub fn core(&self) -> Arc<RuntimeCore> {
        Arc::clone(&self.core)
    }
}

pub struct RuntimeBuilder {
    config: RuntimeConfig,
    drivers: HashMap<String, Arc<dyn PlatformDriver>>,
    snapshots: Option<Arc<dyn SnapshotStore>>,
    sessions: Arc<dyn SessionStore>,
    event_sink: Arc<dyn EventSink>,
}

impl RuntimeBuilder {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            drivers: HashMap::new(),
            snapshots: None,
            sessions: Arc::new(NullSessionStore),
            event_sink: Arc::new(NullEventSink),
        }
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
        self.drivers
            .insert(driver.platform_id().to_string(), driver);
        self
    }

    pub async fn build(self) -> Result<Runtime, OperatorError> {
        let snapshots = self.snapshots.ok_or_else(|| {
            OperatorError::Platform("runtime builder requires a snapshot store".into())
        })?;
        snapshots.evict_expired().await?;

        let default_target = self.config.default_target.clone();
        let core = RuntimeCore {
            resolver: TargetResolver::new(default_target),
            drivers: self.drivers,
            snapshots,
            sessions: self.sessions,
            event_sink: self.event_sink,
            config: self.config,
        };

        Ok(Runtime {
            core: Arc::new(core),
        })
    }
}
