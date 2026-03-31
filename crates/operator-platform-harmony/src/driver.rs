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
    pub fn new(target_id: TargetId, config: HarmonyHdcConfig) -> Self {
        let artifacts_dir = default_artifacts_dir();
        Self {
            worker: Arc::new(HarmonyHdcWorker::new(
                target_id,
                config,
                cache_root_from_artifacts_dir(&artifacts_dir),
            )),
            artifacts_dir,
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
        req: ActionRequest,
        _ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        self.worker.act(req).await
    }
}

fn harmony_capabilities() -> CapabilitySet {
    CapabilitySet::new([
        Capability::Capture,
        Capability::PointerInput,
        Capability::KeyboardInput,
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

fn cache_root_from_artifacts_dir(artifacts_dir: &std::path::Path) -> PathBuf {
    match artifacts_dir.file_name().and_then(|name| name.to_str()) {
        Some("artifacts") => artifacts_dir
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| artifacts_dir.to_path_buf()),
        _ => artifacts_dir.to_path_buf(),
    }
}
