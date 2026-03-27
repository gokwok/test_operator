use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    ActionOutcome, ActionRequest, CapabilitySet, ExecContext, ObserveRequest, ObserveResult,
    OperatorError, PermissionsReport, QueryRequest, QueryResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: Option<String>,
    pub permissions: PermissionsReport,
}

#[async_trait]
pub trait PlatformDriver: Send + Sync {
    fn platform_id(&self) -> &'static str;
    fn driver_id(&self) -> &str;
    fn capabilities(&self) -> CapabilitySet;
    async fn health_check(&self) -> Result<HealthStatus, OperatorError>;
    async fn observe(
        &self,
        req: ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError>;
    async fn query(
        &self,
        req: QueryRequest,
        ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError>;
    async fn act(
        &self,
        req: ActionRequest,
        ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError>;
}
