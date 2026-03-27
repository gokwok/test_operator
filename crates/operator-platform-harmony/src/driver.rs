use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use operator_core::{
    ActionOutcome, ActionRequest, Capability, CapabilitySet, ExecContext, HealthStatus,
    ObserveRequest, ObserveResult, OperatorError, PlatformDriver, QueryRequest, QueryResult,
    TargetId,
};

use crate::{
    observe::observe as observe_with_screenshot,
    permissions::{health_message, health_ready},
    query::query as query_surface,
    HarmonyHdcConfig, HarmonyHdcWorker,
};

const DRIVER_ID: &str = "harmony.hdc";

#[derive(Debug)]
pub struct HarmonyHdcDriver {
    worker: Arc<HarmonyHdcWorker>,
    artifacts_dir: PathBuf,
}

impl HarmonyHdcDriver {
    pub fn new(_target_id: TargetId, config: HarmonyHdcConfig) -> Self {
        Self {
            worker: Arc::new(HarmonyHdcWorker::new(config)),
            artifacts_dir: default_artifacts_dir(),
        }
    }

    pub(crate) fn new_with_worker_and_artifacts_dir(
        worker: Arc<HarmonyHdcWorker>,
        artifacts_dir: PathBuf,
    ) -> Self {
        Self {
            worker,
            artifacts_dir,
        }
    }

    pub fn config(&self) -> &HarmonyHdcConfig {
        self.worker.config()
    }

    pub fn worker(&self) -> &Arc<HarmonyHdcWorker> {
        &self.worker
    }

    pub fn artifacts_dir(&self) -> &std::path::Path {
        &self.artifacts_dir
    }
}

#[async_trait]
impl PlatformDriver for HarmonyHdcDriver {
    fn platform_id(&self) -> &'static str {
        "harmony"
    }

    fn driver_id(&self) -> &str {
        DRIVER_ID
    }

    fn capabilities(&self) -> CapabilitySet {
        harmony_capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        let permissions = self.worker.permissions_report().await?;
        let healthy = health_ready(&permissions);

        Ok(HealthStatus {
            healthy,
            message: health_message(&permissions),
            permissions,
        })
    }

    async fn observe(
        &self,
        req: ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        observe_with_screenshot(self.worker.as_ref(), &self.artifacts_dir, req, ctx).await
    }

    async fn query(
        &self,
        req: QueryRequest,
        _ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError> {
        query_surface(self.worker.as_ref(), req, self.capabilities()).await
    }

    async fn act(
        &self,
        _req: ActionRequest,
        _ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        Err(unimplemented_surface_error("act"))
    }
}

fn unimplemented_surface_error(surface: &str) -> OperatorError {
    OperatorError::Platform(format!(
        "driver {DRIVER_ID} scaffold does not implement {surface} yet"
    ))
}

fn harmony_capabilities() -> CapabilitySet {
    CapabilitySet::new([
        Capability::Capture,
        Capability::AppLifecycle,
        Capability::WindowQuery,
        Capability::Permissions,
    ])
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
