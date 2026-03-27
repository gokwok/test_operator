use std::sync::Arc;

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor};
use operator_runtime::PlatformDriverFactory;

use crate::{HarmonyHdcConfig, HarmonyHdcDriver, HarmonyHdcSessionFactory, HarmonyHdcWorker};

#[derive(Clone)]
pub struct HarmonyHdcDriverFactory {
    session_factory: Arc<dyn HarmonyHdcSessionFactory>,
}

impl HarmonyHdcDriverFactory {
    pub fn new() -> Self {
        Self {
            session_factory: Arc::new(crate::worker::RealHarmonyHdcSessionFactory),
        }
    }

    pub fn new_with_session_factory(session_factory: Arc<dyn HarmonyHdcSessionFactory>) -> Self {
        Self { session_factory }
    }
}

impl Default for HarmonyHdcDriverFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HarmonyHdcDriverFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HarmonyHdcDriverFactory")
    }
}

impl PlatformDriverFactory for HarmonyHdcDriverFactory {
    fn driver_id(&self) -> &str {
        "harmony.hdc"
    }

    fn build(&self, target: &TargetDescriptor) -> Result<Arc<dyn PlatformDriver>, OperatorError> {
        if target.platform != "harmony" {
            return Err(OperatorError::Platform(format!(
                "target {} resolved to platform {}, but harmony.hdc only supports harmony",
                target.id, target.platform
            )));
        }

        if target.driver != self.driver_id() {
            return Err(OperatorError::Platform(format!(
                "target {} resolved to driver {}, but factory {} only supports {}",
                target.id,
                target.driver,
                self.driver_id(),
                self.driver_id()
            )));
        }

        let config = HarmonyHdcConfig::try_from(&target.driver_config).map_err(|error| {
            OperatorError::Platform(format!(
                "invalid driver_config for target {}: {error}",
                target.id
            ))
        })?;
        let worker = Arc::new(HarmonyHdcWorker::new_with_session_factory(
            config,
            Arc::clone(&self.session_factory),
        ));

        Ok(Arc::new(HarmonyHdcDriver::new_with_worker(
            target.id.clone(),
            worker,
        )))
    }
}
