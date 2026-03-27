use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor};
use operator_runtime::PlatformDriverFactory;

use crate::{HarmonyHdcConfig, HarmonyHdcDriver, HarmonyHdcSessionFactory, HarmonyHdcWorker};

#[derive(Clone)]
pub struct HarmonyHdcDriverFactory {
    session_factory: Arc<dyn HarmonyHdcSessionFactory>,
    artifacts_dir: PathBuf,
}

impl HarmonyHdcDriverFactory {
    pub fn new() -> Self {
        Self::new_with_artifacts_dir(default_artifacts_dir())
    }

    pub fn new_with_artifacts_dir(artifacts_dir: impl AsRef<Path>) -> Self {
        Self::new_with_session_factory_and_artifacts_dir(
            Arc::new(crate::worker::RealHarmonyHdcSessionFactory),
            artifacts_dir,
        )
    }

    pub fn new_with_session_factory(session_factory: Arc<dyn HarmonyHdcSessionFactory>) -> Self {
        Self::new_with_session_factory_and_artifacts_dir(session_factory, default_artifacts_dir())
    }

    pub fn new_with_session_factory_and_artifacts_dir(
        session_factory: Arc<dyn HarmonyHdcSessionFactory>,
        artifacts_dir: impl AsRef<Path>,
    ) -> Self {
        Self {
            session_factory,
            artifacts_dir: artifacts_dir.as_ref().to_path_buf(),
        }
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

        Ok(Arc::new(
            HarmonyHdcDriver::new_with_worker_and_artifacts_dir(worker, self.artifacts_dir.clone()),
        ))
    }
}

fn default_artifacts_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("OPERATOR_HOME") {
        return PathBuf::from(path).join("artifacts");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".operator").join("artifacts");
    }

    PathBuf::from(".operator").join("artifacts")
}
