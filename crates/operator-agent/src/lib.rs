//! Agent entry-layer scaffolding for Operator.

pub mod config;
pub mod error;
pub mod harness;
pub mod journal;
pub mod model;
pub mod planner;
pub mod policy;
pub mod progress;
pub mod runner;
pub mod session;
pub mod tools;

use operator_core::{SessionId, TargetId};
use serde::{Deserialize, Serialize};

pub use config::{AgentConfig, PlannerFormat};
pub use error::AgentError;
pub use harness::{
    render_harness_report, summarize_timing, summarize_transcript_replay, HarnessReplaySummary,
    HarnessReport, HarnessTimingSummary, ToolTimingSummary,
};
pub use journal::{load_persisted_session, PersistedSessionTranscript, ReplayableTranscriptEvent};
pub use planner::{AgentDecision, DecisionParser, DecisionValidator};
pub use planner::{FinishGate, FinishGateVerdict};
pub use policy::{
    PlannerFailureStage, PlannerRetryDecision, PlannerRetryPolicy, RepeatedErrorDecision,
    RepeatedErrorPolicy,
};
pub use progress::{AgentProgressEvent, AgentProgressReporter, NoopAgentProgressReporter};
pub use runner::AgentRunner;
pub use session::{
    AgentMessage, AgentSessionState, AgentSessionStatus, LoopHistoryItem, LoopState,
    ModelContextBuffer, SessionJournal, ToolTraceEntry, VisualObservationSummary,
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
