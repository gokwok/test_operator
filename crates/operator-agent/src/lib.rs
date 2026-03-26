//! Agent entry-layer scaffolding for Operator.

pub mod config;
pub mod error;
pub mod model;
pub mod planner;
pub mod policy;
pub mod runner;
pub mod session;
pub mod tools;

use operator_core::{SessionId, TargetId};
use serde::{Deserialize, Serialize};

pub use config::{AgentConfig, PlannerFormat};
pub use error::AgentError;
pub use planner::{AgentDecision, DecisionParser, DecisionValidator};
pub use planner::{FinishGate, FinishGateVerdict};
pub use policy::{
    PlannerFailureStage, PlannerRetryDecision, PlannerRetryPolicy, RepeatedErrorDecision,
    RepeatedErrorPolicy,
};
pub use runner::AgentRunner;
pub use session::{
    load_persisted_session, AgentMessage, AgentSessionState, AgentSessionStatus, LoopHistoryItem,
    LoopState, PersistedSessionTranscript, ReplayableTranscriptEvent, SessionJournal,
    ToolTraceEntry, VisualObservationSummary,
};

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
