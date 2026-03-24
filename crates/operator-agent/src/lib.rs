//! Agent entry-layer scaffolding for Operator.

pub mod config;
pub mod error;
pub mod model;

use operator_core::{SessionId, TargetId};
use serde::{Deserialize, Serialize};

pub use config::{AgentConfig, PlannerFormat};
pub use error::AgentError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunRequest {
    pub task: String,
    pub target: TargetId,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub session_id: SessionId,
    pub target: TargetId,
    pub model: String,
    pub summary: String,
}
