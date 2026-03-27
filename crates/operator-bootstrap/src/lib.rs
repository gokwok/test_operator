use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor};
use operator_platform_macos::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};
use operator_runtime::{PlatformDriverFactory, PlatformRegistry};

pub fn system_platform_registry(artifacts_dir: impl AsRef<Path>) -> PlatformRegistry {
    let mut registry = PlatformRegistry::new();
    registry.register_factory(Arc::new(MacosSystemDriverFactory::new(artifacts_dir)));
    registry
}

struct MacosSystemDriverFactory {
    artifacts_dir: PathBuf,
}

impl MacosSystemDriverFactory {
    fn new(artifacts_dir: impl AsRef<Path>) -> Self {
        Self {
            artifacts_dir: artifacts_dir.as_ref().to_path_buf(),
        }
    }
}

impl PlatformDriverFactory for MacosSystemDriverFactory {
    fn driver_id(&self) -> &str {
        "macos.system"
    }

    fn build(&self, target: &TargetDescriptor) -> Result<Arc<dyn PlatformDriver>, OperatorError> {
        if target.platform != "macos" {
            return Err(OperatorError::Platform(format!(
                "target {} resolved to platform {}, but macos.system only supports macos",
                target.id, target.platform
            )));
        }

        if !target.driver_config.is_empty() {
            return Err(OperatorError::Platform(format!(
                "driver macos.system does not accept target-level driver_config: {}",
                serde_json::to_string(&target.driver_config)?
            )));
        }

        Ok(Arc::new(MacosDriver::with_observe(
            SystemAppService,
            SystemPermissionReader,
            SystemCaptureProvider::new(&self.artifacts_dir),
            SystemTreeInspector,
        )) as Arc<dyn PlatformDriver>)
    }
}
