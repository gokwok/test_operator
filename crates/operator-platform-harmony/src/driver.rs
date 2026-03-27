use std::sync::Arc;

use async_trait::async_trait;
use operator_core::{
    ActionOutcome, ActionRequest, CapabilitySet, ExecContext, HealthStatus, ObserveRequest,
    ObserveResult, OperatorError, PermissionsReport, PlatformDriver, QueryRequest, QueryResult,
    TargetId,
};

use crate::{HarmonyHdcConfig, HarmonyHdcWorker};

const DRIVER_ID: &str = "harmony.hdc";

#[derive(Debug)]
pub struct HarmonyHdcDriver {
    target_id: TargetId,
    config: HarmonyHdcConfig,
    worker: Arc<HarmonyHdcWorker>,
}

impl HarmonyHdcDriver {
    pub fn new(target_id: TargetId, config: HarmonyHdcConfig) -> Self {
        let worker = Arc::new(HarmonyHdcWorker::new(config.clone()));
        Self {
            target_id,
            config,
            worker,
        }
    }

    pub fn config(&self) -> &HarmonyHdcConfig {
        &self.config
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
        Ok(HealthStatus {
            healthy: false,
            message: Some(format!(
                "target {} is wired to {}, but the concrete harmony action/query surface is not implemented yet",
                self.target_id, DRIVER_ID
            )),
            permissions: PermissionsReport { checks: Vec::new() },
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
