use std::sync::Arc;

use operator_core::{ArtifactId, SessionId, Snapshot, SnapshotId, SurfaceKind, TargetId};
use operator_runtime::{Session, SessionEvent, SessionStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    model::Message,
    tools::{AgentToolResult, ObservationCache},
    AgentError,
};

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
pub struct VisualObservationSummary {
    pub snapshot_id: SnapshotId,
    pub surface: String,
    pub screenshot_artifact: Option<ArtifactId>,
    pub root_element_count: usize,
    pub element_count: usize,
}

impl VisualObservationSummary {
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            snapshot_id: snapshot.id.clone(),
            surface: surface_name(&snapshot.surface.kind),
            screenshot_artifact: snapshot.image_artifact.clone(),
            root_element_count: snapshot.root_ids.len(),
            element_count: snapshot.elements.len(),
        }
    }

    pub fn describe(&self) -> String {
        let screenshot = self
            .screenshot_artifact
            .as_ref()
            .map(|artifact| format!(", screenshot={artifact}"))
            .unwrap_or_default();
        format!(
            "observe snapshot {} on {} (roots={}, elements={}){}",
            self.snapshot_id, self.surface, self.root_element_count, self.element_count, screenshot
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopHistoryItem {
    pub turn_index: u32,
    pub step_index: u32,
    pub kind: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoopState {
    pub session_id: SessionId,
    pub target: TargetId,
    pub task: String,
    pub status: AgentSessionStatus,
    pub turn_index: u32,
    pub step_index: u32,
    pub parse_attempts: u32,
    pub messages: Vec<AgentMessage>,
    pub history: Vec<LoopHistoryItem>,
    pub tool_trace: Vec<ToolTraceEntry>,
    pub notes: Vec<String>,
    pub current_observation: Option<VisualObservationSummary>,
    pub observation_cache: ObservationCache,
    pub latest_snapshot: Option<SnapshotId>,
    pub previous_snapshot_visual: Option<ArtifactId>,
    pub latest_artifacts: Vec<ArtifactId>,
    pub ui_state_stale: bool,
    pub consecutive_error_count: u32,
    pub last_error_fingerprint: Option<String>,
}

pub type AgentSessionState = LoopState;

impl LoopState {
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
            history: Vec::new(),
            tool_trace: Vec::new(),
            notes: Vec::new(),
            current_observation: None,
            observation_cache: ObservationCache::new(),
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
        self.history.clear();
        self.tool_trace.clear();
        self.notes.clear();
        self.current_observation = None;
        self.observation_cache.clear();
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
        if result.tool_name != "observe" || result.is_error {
            self.history.push(LoopHistoryItem {
                turn_index: self.turn_index,
                step_index: self.step_index,
                kind: "tool_result".into(),
                summary: history_summary_from_tool_result(&result),
            });
        }
        self.tool_trace.push(ToolTraceEntry {
            turn_index: self.turn_index,
            step_index: self.step_index,
            timestamp_ms,
            result,
        });
    }

    pub fn add_note(&mut self, note: impl Into<String>) {
        let note = note.into();
        self.history.push(LoopHistoryItem {
            turn_index: self.turn_index,
            step_index: self.step_index,
            kind: "note".into(),
            summary: note.clone(),
        });
        self.notes.push(note);
    }

    pub fn mark_ui_stale(&mut self) {
        self.ui_state_stale = true;
    }

    fn update_ui_state_staleness(&mut self, result: &AgentToolResult) {
        if result.is_error {
            return;
        }

        if result.tool_name == "observe" {
            self.ui_state_stale = !observe_result_is_usable(result);
        } else if !result.read_only {
            self.ui_state_stale = true;
        }
    }

    pub fn record_visual_observation(&mut self, summary: VisualObservationSummary) {
        self.latest_snapshot = Some(summary.snapshot_id.clone());
        self.observation_cache.record(summary.clone());
        self.current_observation = Some(summary.clone());
        self.previous_snapshot_visual = self.observation_cache.previous_visual().cloned();
        self.latest_artifacts = summary.screenshot_artifact.clone().into_iter().collect();
        self.history.push(LoopHistoryItem {
            turn_index: self.turn_index,
            step_index: self.step_index,
            kind: "observation".into(),
            summary: summary.describe(),
        });
    }

    pub fn record_observation_snapshot(&mut self, snapshot: &Snapshot) {
        self.record_visual_observation(VisualObservationSummary::from_snapshot(snapshot));
    }

    pub fn record_observation(
        &mut self,
        snapshot_id: SnapshotId,
        artifacts: Vec<ArtifactId>,
        visual: Option<ArtifactId>,
    ) {
        self.observation_cache.record(VisualObservationSummary {
            snapshot_id: snapshot_id.clone(),
            surface: "unknown".into(),
            screenshot_artifact: visual.clone(),
            root_element_count: 0,
            element_count: 0,
        });
        self.current_observation = self.observation_cache.current_observation().cloned();
        self.latest_snapshot = Some(snapshot_id);
        self.latest_artifacts = artifacts;
        self.previous_snapshot_visual = self.observation_cache.previous_visual().cloned();
    }

    pub fn current_observation(&self) -> Option<&VisualObservationSummary> {
        self.current_observation.as_ref()
    }

    pub fn current_visual(&self) -> Option<&ArtifactId> {
        self.observation_cache.current_visual()
    }

    pub fn previous_visual(&self) -> Option<&ArtifactId> {
        self.observation_cache.previous_visual()
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

fn history_summary_from_tool_result(result: &AgentToolResult) -> String {
    if result.is_error {
        return result
            .error
            .as_ref()
            .map(|error| format!("tool {} failed: {}", result.tool_name, error.message))
            .unwrap_or_else(|| format!("tool {} failed", result.tool_name));
    }

    if result.tool_name == "observe" {
        if let Some(snapshot) = result
            .output
            .as_ref()
            .and_then(|output| output.get("snapshot"))
            .cloned()
            .and_then(|snapshot| serde_json::from_value::<Snapshot>(snapshot).ok())
        {
            return VisualObservationSummary::from_snapshot(&snapshot).describe();
        }
    }

    format!("tool {} completed", result.tool_name)
}

fn surface_name(kind: &SurfaceKind) -> String {
    match kind {
        SurfaceKind::Fullscreen { .. } => "fullscreen".into(),
        SurfaceKind::Frontmost => "frontmost".into(),
        SurfaceKind::Window { .. } => "window".into(),
        SurfaceKind::Region { .. } => "region".into(),
    }
}

fn observe_result_is_usable(result: &AgentToolResult) -> bool {
    if result.tool_name != "observe" || result.is_error {
        return false;
    }

    let include_elements = result
        .arguments
        .get("include_elements")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !include_elements {
        return false;
    }

    let Some(snapshot) = result
        .output
        .as_ref()
        .and_then(|output| output.get("snapshot"))
        .and_then(Value::as_object)
    else {
        return false;
    };

    let root_count = snapshot
        .get("root_ids")
        .and_then(Value::as_array)
        .map_or(0, |items| items.len());
    let element_count = snapshot
        .get("elements")
        .and_then(Value::as_object)
        .map_or(0, |items| items.len());

    root_count > 0 && element_count > 0
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
