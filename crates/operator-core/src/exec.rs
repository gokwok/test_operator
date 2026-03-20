use serde::{Deserialize, Serialize};

use crate::{SessionId, Snapshot, Surface, TargetId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecContext {
    pub target: TargetId,
    pub session: Option<SessionId>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveRequest {
    pub surface: Surface,
    pub include_screenshot: bool,
    pub include_elements: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveResult {
    pub snapshot: Snapshot,
}
