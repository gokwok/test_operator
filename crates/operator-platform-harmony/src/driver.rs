use std::sync::Arc;

use async_trait::async_trait;
use operator_core::{
    ActionOutcome, ActionRequest, CapabilitySet, ExecContext, HealthStatus, ObserveRequest,
    ObserveResult, OperatorError, PlatformDriver, QueryRequest, QueryResult, TargetId,
};

use crate::{
    permissions::{health_message, health_ready},
    HarmonyHdcConfig, HarmonyHdcWorker,
};

const DRIVER_ID: &str = "harmony.hdc";

#[derive(Debug)]
pub struct HarmonyHdcDriver {
    target_id: TargetId,
    worker: Arc<HarmonyHdcWorker>,
}

impl HarmonyHdcDriver {
    pub fn new(target_id: TargetId, config: HarmonyHdcConfig) -> Self {
        Self::new_with_worker(target_id, Arc::new(HarmonyHdcWorker::new(config)))
    }

    pub(crate) fn new_with_worker(target_id: TargetId, worker: Arc<HarmonyHdcWorker>) -> Self {
        Self { target_id, worker }
    }

    pub fn config(&self) -> &HarmonyHdcConfig {
        self.worker.config()
    }

    pub fn worker(&self) -> &Arc<HarmonyHdcWorker> {
        &self.worker
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
        CapabilitySet::default()
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        let permissions = self.worker.permissions_report().await?;
        let healthy = health_ready(&permissions);

        Ok(HealthStatus {
            healthy,
            message: health_message(&permissions).or_else(|| {
                Some(format!(
                    "target {} is wired to {}, but observe/query/action are not implemented yet",
                    self.target_id, DRIVER_ID
                ))
            }),
            permissions,
        })
    }

    async fn observe(
        &self,
        _req: ObserveRequest,
        _ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        Err(unimplemented_surface_error("observe"))
    }

    async fn query(
        &self,
        _req: QueryRequest,
        _ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError> {
        Err(unimplemented_surface_error("query"))
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
