use operator_core::{SessionId, TargetId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentProgressEvent {
    RunStarted {
        session_id: SessionId,
        target: TargetId,
        model: String,
        task: String,
    },
    TurnStarted {
        turn_index: u32,
    },
    PlannedTool {
        turn_index: u32,
        tool_name: String,
        summary: String,
    },
    FinishPlanned {
        turn_index: u32,
        summary: String,
    },
    ToolCall {
        turn_index: u32,
        step_index: u32,
        name: String,
        args: Value,
    },
    ToolResult {
        turn_index: u32,
        step_index: u32,
        name: String,
        summary: String,
        is_error: bool,
    },
    FinishGateRejected {
        turn_index: u32,
        reason: String,
    },
    RunCompleted {
        summary: String,
    },
    RunFailed {
        reason: String,
    },
}

pub trait AgentProgressReporter: Send + Sync {
    fn report(&self, event: AgentProgressEvent);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAgentProgressReporter;

impl AgentProgressReporter for NoopAgentProgressReporter {
    fn report(&self, _: AgentProgressEvent) {}
}
