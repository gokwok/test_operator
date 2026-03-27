use std::{path::Path, sync::Arc};

use operator_core::PlatformDriver;

use crate::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};

pub fn system_runtime_drivers(artifacts_dir: impl AsRef<Path>) -> Vec<Arc<dyn PlatformDriver>> {
    vec![Arc::new(MacosDriver::with_observe(
        SystemAppService,
        SystemPermissionReader,
        SystemCaptureProvider::new(artifacts_dir),
        SystemTreeInspector,
    )) as Arc<dyn PlatformDriver>]
}
