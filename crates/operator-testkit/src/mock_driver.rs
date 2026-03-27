use std::{collections::VecDeque, sync::Mutex};

use async_trait::async_trait;
use operator_core::{
    ActionOutcome, ActionRequest, CapabilitySet, ExecContext, HealthStatus, ObserveRequest,
    ObserveResult, OperatorError, PermissionStatus, PermissionsReport, PlatformDriver,
    QueryRequest, QueryResult,
};

// Keep the future PlatformDriver method shape without pulling runtime abstractions
// forward before the runtime builder/execution issues land.
pub struct MockPlatformDriver {
    platform_id: &'static str,
    driver_id: String,
    capabilities: CapabilitySet,
    health_status: Mutex<HealthStatus>,
    observe_results: Mutex<VecDeque<Result<ObserveResult, OperatorError>>>,
    query_results: Mutex<VecDeque<Result<QueryResult, OperatorError>>>,
    action_results: Mutex<VecDeque<Result<ActionOutcome, OperatorError>>>,
    observe_calls: Mutex<Vec<(ObserveRequest, ExecContext)>>,
    query_calls: Mutex<Vec<(QueryRequest, ExecContext)>>,
    action_calls: Mutex<Vec<(ActionRequest, ExecContext)>>,
}

impl MockPlatformDriver {
    pub fn new(platform_id: &'static str, capabilities: CapabilitySet) -> Self {
        Self::with_driver_id(platform_id, format!("{platform_id}.system"), capabilities)
    }

    pub fn with_driver_id(
        platform_id: &'static str,
        driver_id: impl Into<String>,
        capabilities: CapabilitySet,
    ) -> Self {
        Self {
            platform_id,
            driver_id: driver_id.into(),
            capabilities,
            health_status: Mutex::new(HealthStatus {
                healthy: true,
                message: None,
                permissions: PermissionsReport {
                    accessibility: PermissionStatus::Granted,
                    system_events: PermissionStatus::Granted,
                    screen_recording: PermissionStatus::Granted,
                },
            }),
            observe_results: Mutex::new(VecDeque::new()),
            query_results: Mutex::new(VecDeque::new()),
            action_results: Mutex::new(VecDeque::new()),
            observe_calls: Mutex::new(Vec::new()),
            query_calls: Mutex::new(Vec::new()),
            action_calls: Mutex::new(Vec::new()),
        }
    }

    pub fn platform_id(&self) -> &'static str {
        self.platform_id
    }

    pub fn capabilities(&self) -> CapabilitySet {
        self.capabilities.clone()
    }

    pub fn driver_id(&self) -> &str {
        &self.driver_id
    }

    pub fn set_health_status(&self, status: HealthStatus) {
        *self.health_status.lock().unwrap() = status;
    }

    pub async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        Ok(self.health_status.lock().unwrap().clone())
    }

    pub fn push_observe_result(&self, result: Result<ObserveResult, OperatorError>) {
        self.observe_results.lock().unwrap().push_back(result);
    }

    pub fn push_query_result(&self, result: Result<QueryResult, OperatorError>) {
        self.query_results.lock().unwrap().push_back(result);
    }

    pub fn push_action_result(&self, result: Result<ActionOutcome, OperatorError>) {
        self.action_results.lock().unwrap().push_back(result);
    }

    pub async fn observe(
        &self,
        req: ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        self.observe_calls.lock().unwrap().push((req, ctx.clone()));

        self.observe_results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(OperatorError::Platform(format!(
                    "no mocked observe response queued for {}",
                    self.platform_id
                )))
            })
    }

    pub async fn query(
        &self,
        req: QueryRequest,
        ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError> {
        self.query_calls.lock().unwrap().push((req, ctx.clone()));

        self.query_results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(OperatorError::Platform(format!(
                    "no mocked query response queued for {}",
                    self.platform_id
                )))
            })
    }

    pub async fn act(
        &self,
        req: ActionRequest,
        ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        self.action_calls.lock().unwrap().push((req, ctx.clone()));

        self.action_results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(OperatorError::Platform(format!(
                    "no mocked action response queued for {}",
                    self.platform_id
                )))
            })
    }

    pub async fn observe_calls(&self) -> Vec<(ObserveRequest, ExecContext)> {
        self.observe_calls.lock().unwrap().clone()
    }

    pub async fn query_calls(&self) -> Vec<(QueryRequest, ExecContext)> {
        self.query_calls.lock().unwrap().clone()
    }

    pub async fn action_calls(&self) -> Vec<(ActionRequest, ExecContext)> {
        self.action_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl PlatformDriver for MockPlatformDriver {
    fn platform_id(&self) -> &'static str {
        self.platform_id()
    }

    fn driver_id(&self) -> &str {
        self.driver_id()
    }

    fn capabilities(&self) -> CapabilitySet {
        self.capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        MockPlatformDriver::health_check(self).await
    }

    async fn observe(
        &self,
        req: ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        MockPlatformDriver::observe(self, req, ctx).await
    }

    async fn query(
        &self,
        req: QueryRequest,
        ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError> {
        MockPlatformDriver::query(self, req, ctx).await
    }

    async fn act(
        &self,
        req: ActionRequest,
        ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        MockPlatformDriver::act(self, req, ctx).await
    }
}
