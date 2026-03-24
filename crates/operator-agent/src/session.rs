use std::sync::Arc;

use operator_core::{ArtifactId, SessionId, SnapshotId, TargetId};
use operator_runtime::{Session, SessionEvent, SessionStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{model::Message, tools::AgentToolResult, AgentError};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum AgentMessage {
    Base { message: Message },
    Custom { kind: Arc<str>, payload: Value },
}

impl AgentMessage {
    pub fn as_model_message(&self) -> Option<&Message> {
        match self {
            Self::Base { message } => Some(message),
            Self::Custom { .. } => None,
        }
    }

    pub fn custom(kind: impl Into<Arc<str>>, payload: Value) -> Self {
        Self::Custom {
            kind: kind.into(),
            payload,
        }
    }
}

impl From<Message> for AgentMessage {
    fn from(value: Message) -> Self {
        Self::Base { message: value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentSessionStatus {
    Running,
    Completed { summary: String },
    Failed { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolTraceEntry {
    pub turn_index: u32,
    pub step_index: u32,
    pub timestamp_ms: u64,
    pub result: AgentToolResult,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentSessionState {
    pub session_id: SessionId,
    pub target: TargetId,
    pub task: String,
    pub status: AgentSessionStatus,
    pub turn_index: u32,
    pub step_index: u32,
    pub parse_attempts: u32,
    pub messages: Vec<AgentMessage>,
    pub tool_trace: Vec<ToolTraceEntry>,
    pub notes: Vec<String>,
    pub latest_snapshot: Option<SnapshotId>,
    pub previous_snapshot_visual: Option<ArtifactId>,
    pub latest_artifacts: Vec<ArtifactId>,
    pub ui_state_stale: bool,
    pub consecutive_error_count: u32,
    pub last_error_fingerprint: Option<String>,
}

impl AgentSessionState {
    pub fn new(session_id: SessionId, target: TargetId, task: impl Into<String>) -> Self {
        Self {
            session_id,
            target,
            task: task.into(),
            status: AgentSessionStatus::Running,
            turn_index: 0,
            step_index: 0,
            parse_attempts: 0,
            messages: Vec::new(),
            tool_trace: Vec::new(),
            notes: Vec::new(),
            latest_snapshot: None,
            previous_snapshot_visual: None,
            latest_artifacts: Vec::new(),
            ui_state_stale: false,
            consecutive_error_count: 0,
            last_error_fingerprint: None,
        }
    }

    pub fn start_turn(&mut self) {
        self.turn_index += 1;
        self.step_index = 0;
        self.parse_attempts = 0;
    }

    pub fn bootstrap_task(&mut self, task: impl Into<String>) {
        self.task = task.into();
        self.status = AgentSessionStatus::Running;
        self.turn_index = 0;
        self.step_index = 0;
        self.parse_attempts = 0;
        self.messages.clear();
        self.tool_trace.clear();
        self.notes.clear();
        self.latest_snapshot = None;
        self.previous_snapshot_visual = None;
        self.latest_artifacts.clear();
        self.ui_state_stale = false;
        self.clear_error_tracking();
    }

    pub fn start_step(&mut self) {
        self.step_index += 1;
        self.parse_attempts = 0;
    }

    pub fn bump_parse_attempts(&mut self) -> u32 {
        self.parse_attempts += 1;
        self.parse_attempts
    }

    pub fn push_message(&mut self, message: impl Into<AgentMessage>) {
        self.messages.push(message.into());
    }

    pub fn push_tool_trace(&mut self, result: AgentToolResult, timestamp_ms: u64) {
        self.update_ui_state_staleness(&result);
        self.tool_trace.push(ToolTraceEntry {
            turn_index: self.turn_index,
            step_index: self.step_index,
            timestamp_ms,
            result,
        });
    }

    pub fn add_note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    pub fn mark_ui_stale(&mut self) {
        self.ui_state_stale = true;
    }

    fn update_ui_state_staleness(&mut self, result: &AgentToolResult) {
        if result.is_error {
            return;
        }

        if result.tool_name == "observe" {
            self.ui_state_stale = false;
        } else if !result.read_only {
            self.ui_state_stale = true;
        }
    }

    pub fn record_observation(
        &mut self,
        snapshot_id: SnapshotId,
        artifacts: Vec<ArtifactId>,
        visual: Option<ArtifactId>,
    ) {
        self.latest_snapshot = Some(snapshot_id);
        self.latest_artifacts = artifacts;
        self.previous_snapshot_visual = visual;
        self.ui_state_stale = false;
    }

    pub fn record_error_fingerprint(&mut self, fingerprint: impl Into<String>) -> u32 {
        let fingerprint = fingerprint.into();
        if self.last_error_fingerprint.as_deref() == Some(fingerprint.as_str()) {
            self.consecutive_error_count += 1;
        } else {
            self.consecutive_error_count = 1;
            self.last_error_fingerprint = Some(fingerprint);
        }

        self.consecutive_error_count
    }

    pub fn clear_error_tracking(&mut self) {
        self.consecutive_error_count = 0;
        self.last_error_fingerprint = None;
    }

    pub fn complete(&mut self, summary: impl Into<String>) {
        self.status = AgentSessionStatus::Completed {
            summary: summary.into(),
        };
    }

    pub fn fail(&mut self, reason: impl Into<String>) {
        self.status = AgentSessionStatus::Failed {
            reason: reason.into(),
        };
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ReplayableTranscriptEvent {
    UserInput { text: String },
    ToolCall { name: String, input: Value },
    ToolResult { result: AgentToolResult },
    ModelResponse { content: String },
    Completed { summary: Option<String> },
    Error { message: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedSessionTranscript {
    pub session: Session,
    pub events: Vec<ReplayableTranscriptEvent>,
}

impl TryFrom<SessionEvent> for ReplayableTranscriptEvent {
    type Error = operator_core::OperatorError;

    fn try_from(value: SessionEvent) -> Result<Self, operator_core::OperatorError> {
        match value {
            SessionEvent::UserInput { text } => Ok(Self::UserInput { text }),
            SessionEvent::ToolCall { name, input } => Ok(Self::ToolCall { name, input }),
            SessionEvent::ToolResult { output, .. } => Ok(Self::ToolResult {
                result: serde_json::from_value(output)?,
            }),
            SessionEvent::ModelResponse { content } => Ok(Self::ModelResponse { content }),
            SessionEvent::Completed { summary } => Ok(Self::Completed { summary }),
            SessionEvent::Error { message } => Ok(Self::Error { message }),
        }
    }
}

pub async fn load_persisted_session(
    store: &dyn SessionStore,
    id: &SessionId,
) -> Result<Option<PersistedSessionTranscript>, AgentError> {
    let Some(session) = store.get(id).await? else {
        return Ok(None);
    };
    let events = store
        .events(id)
        .await?
        .into_iter()
        .map(ReplayableTranscriptEvent::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(PersistedSessionTranscript { session, events }))
}
