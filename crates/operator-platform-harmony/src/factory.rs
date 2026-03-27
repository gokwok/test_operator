use std::sync::Arc;

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor};
use operator_runtime::PlatformDriverFactory;

use crate::{HarmonyHdcConfig, HarmonyHdcDriver};

#[derive(Debug, Default, Clone, Copy)]
pub struct HarmonyHdcDriverFactory;

impl HarmonyHdcDriverFactory {
    pub fn new() -> Self {
        Self
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

        Ok(Arc::new(HarmonyHdcDriver::new(target.id.clone(), config)))
    }
}
