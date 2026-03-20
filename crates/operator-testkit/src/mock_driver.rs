use std::{collections::VecDeque, sync::Mutex};

use operator_core::{
    ActionOutcome, ActionRequest, CapabilitySet, ExecContext, ObserveRequest, ObserveResult,
    OperatorError, QueryRequest, QueryResult,
};

// Keep the future PlatformDriver method shape without pulling runtime abstractions
// forward before the runtime builder/execution issues land.
pub struct MockPlatformDriver {
    platform_id: &'static str,
    capabilities: CapabilitySet,
    observe_results: Mutex<VecDeque<Result<ObserveResult, OperatorError>>>,
    query_results: Mutex<VecDeque<Result<QueryResult, OperatorError>>>,
    action_results: Mutex<VecDeque<Result<ActionOutcome, OperatorError>>>,
    observe_calls: Mutex<Vec<(ObserveRequest, ExecContext)>>,
    query_calls: Mutex<Vec<(QueryRequest, ExecContext)>>,
    action_calls: Mutex<Vec<(ActionRequest, ExecContext)>>,
}

impl MockPlatformDriver {
    pub fn new(platform_id: &'static str, capabilities: CapabilitySet) -> Self {
        Self {
            platform_id,
            capabilities,
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
