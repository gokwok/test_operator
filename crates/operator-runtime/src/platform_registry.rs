use std::{collections::HashMap, sync::Arc};

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor};

pub trait PlatformDriverFactory: Send + Sync {
    fn driver_id(&self) -> &str;
    fn build(&self, target: &TargetDescriptor) -> Result<Arc<dyn PlatformDriver>, OperatorError>;
}

#[derive(Default, Clone)]
pub struct PlatformRegistry {
    factories: HashMap<String, Arc<dyn PlatformDriverFactory>>,
}

impl PlatformRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_factory(&mut self, factory: Arc<dyn PlatformDriverFactory>) {
        self.factories
            .insert(factory.driver_id().to_string(), factory);
    }

    pub fn extend(&mut self, other: PlatformRegistry) {
        self.factories.extend(other.factories);
    }

    pub fn factory(&self, driver_id: &str) -> Option<Arc<dyn PlatformDriverFactory>> {
        self.factories.get(driver_id).cloned()
    }
}
