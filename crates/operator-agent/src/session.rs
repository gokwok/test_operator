use std::sync::Arc;

use operator_core::{AppInfo, ArtifactId, SessionId, Snapshot, SnapshotId, SurfaceKind, TargetId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    model::{AssistantMessage, ContentBlock, Message, ToolResultMessage, UserMessage},
    tools::{AgentToolResult, ObservationCache},
};

pub use crate::journal::{
    load_persisted_session, PersistedSessionTranscript, ReplayableTranscriptEvent, SessionJournal,
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelContextBuffer {
    messages: Vec<AgentMessage>,
}

const MAX_TEXT_SUMMARY_CHARS: usize = 120;
const MAX_CONTEXT_TEXT_CHARS: usize = 240;

impl ModelContextBuffer {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn messages(&self) -> &[AgentMessage] {
        &self.messages
    }

    pub fn planner_messages(&self, message_limit: usize, char_limit: usize) -> Vec<Message> {
        let mut total_chars = 0;
        let mut selected = Vec::new();

        for message in self.messages.iter().rev() {
            let compact = planner_message(message);
            let compact_chars = message_char_count(&compact);
            if !selected.is_empty()
                && (selected.len() >= message_limit || total_chars + compact_chars > char_limit)
            {
                break;
            }

            total_chars += compact_chars;
            selected.push(compact);
        }

        selected.reverse();
        selected
    }

    pub fn push(&mut self, message: impl Into<AgentMessage>) {
        self.messages.push(message.into());
    }

    pub fn push_tool_result(
        &mut self,
        tool_call_id: Arc<str>,
        result: &AgentToolResult,
        timestamp_ms: u64,
    ) {
        self.push(Message::ToolResult(crate::model::ToolResultMessage {
            tool_call_id,
            tool_name: Arc::<str>::from(result.tool_name.clone()),
            content: vec![crate::model::ContentBlock::Text {
                text: history_summary_from_tool_result(result),
            }],
            is_error: result.is_error,
            timestamp_ms,
        }));
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
            "snapshot {} on {} (roots={}, elements={}){}",
            self.snapshot_id, self.surface, self.root_element_count, self.element_count, screenshot
        )
    }

    pub fn is_usable(&self, include_elements: bool) -> bool {
        if include_elements {
            self.root_element_count > 0 && self.element_count > 0
        } else {
            self.screenshot_artifact.is_some()
                || (self.root_element_count > 0 && self.element_count > 0)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopHistoryItem {
    pub turn_index: u32,
    pub step_index: u32,
    pub kind: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapAppCatalogEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub is_running: bool,
}

impl From<AppInfo> for BootstrapAppCatalogEntry {
    fn from(value: AppInfo) -> Self {
        Self {
            name: value.name,
            bundle_id: value.bundle_id,
            is_running: value.is_running,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapAppCatalog {
    pub total_count: usize,
    pub entries: Vec<BootstrapAppCatalogEntry>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub truncated_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapAppContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prelaunched_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_catalog: Option<BootstrapAppCatalog>,
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
    pub model_context: ModelContextBuffer,
    pub history: Vec<LoopHistoryItem>,
    pub tool_trace: Vec<ToolTraceEntry>,
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "is_bootstrap_app_context_empty")]
    pub app_bootstrap: BootstrapAppContext,
    pub current_observation: Option<VisualObservationSummary>,
    pub observation_cache: ObservationCache,
    pub latest_snapshot: Option<SnapshotId>,
    pub previous_snapshot_visual: Option<ArtifactId>,
    pub latest_artifacts: Vec<ArtifactId>,
    pub include_elements: bool,
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
            model_context: ModelContextBuffer::new(),
            history: Vec::new(),
            tool_trace: Vec::new(),
            notes: Vec::new(),
            app_bootstrap: BootstrapAppContext::default(),
            current_observation: None,
            observation_cache: ObservationCache::new(),
            latest_snapshot: None,
            previous_snapshot_visual: None,
            latest_artifacts: Vec::new(),
            include_elements: true,
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
        self.model_context.clear();
        self.history.clear();
        self.tool_trace.clear();
        self.notes.clear();
        self.app_bootstrap = BootstrapAppContext::default();
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
        self.model_context.push(message);
    }

    pub fn push_tool_result_message(
        &mut self,
        tool_call_id: Arc<str>,
        result: &AgentToolResult,
        timestamp_ms: u64,
    ) {
        self.model_context
            .push_tool_result(tool_call_id, result, timestamp_ms);
    }

    pub fn model_context(&self) -> &ModelContextBuffer {
        &self.model_context
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

    pub fn record_bootstrap_app_catalog(&mut self, catalog: BootstrapAppCatalog) {
        self.app_bootstrap.installed_catalog = Some(catalog);
    }

    pub fn record_prelaunched_app(&mut self, app: impl Into<String>) {
        self.app_bootstrap.prelaunched_app = Some(app.into());
    }

    pub fn mark_ui_stale(&mut self) {
        self.ui_state_stale = true;
    }

    pub fn set_include_elements(&mut self, include_elements: bool) {
        self.include_elements = include_elements;
    }

    pub fn include_elements(&self) -> bool {
        self.include_elements
    }

    fn update_ui_state_staleness(&mut self, result: &AgentToolResult) {
        if result.is_error {
            return;
        }

        if result.tool_name == "observe" {
            self.ui_state_stale = !observe_result_is_usable(result, self.include_elements);
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

    let summary = summarize_tool_result(result);
    if result.tool_name == "observe" {
        summary
    } else {
        format!("{}: {}", result.tool_name, summary)
    }
}

fn surface_name(kind: &SurfaceKind) -> String {
    match kind {
        SurfaceKind::Fullscreen { .. } => "fullscreen".into(),
        SurfaceKind::Frontmost => "frontmost".into(),
        SurfaceKind::Window { .. } => "window".into(),
        SurfaceKind::Region { .. } => "region".into(),
    }
}

fn observe_result_is_usable(result: &AgentToolResult, include_elements: bool) -> bool {
    if result.tool_name != "observe" || result.is_error {
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
    let screenshot_artifact = snapshot
        .get("image_artifact")
        .and_then(Value::as_str)
        .map(str::to_owned);

    VisualObservationSummary {
        snapshot_id: SnapshotId("unknown".into()),
        surface: "unknown".into(),
        screenshot_artifact: screenshot_artifact.map(ArtifactId),
        root_element_count: root_count,
        element_count,
    }
    .is_usable(include_elements)
}

pub(crate) fn summarize_tool_result(result: &AgentToolResult) -> String {
    if result.is_error {
        return result
            .error
            .as_ref()
            .map(|error| truncate(&format!("error [{}]: {}", error.kind, error.message)))
            .unwrap_or_else(|| "tool returned an unknown error".into());
    }

    let Some(output) = result.output.as_ref() else {
        return "completed without structured output".into();
    };

    if let Some(summary) = output.get("snapshot").and_then(snapshot_summary) {
        return summary;
    }

    if let Some(artifact_id) = output
        .get("artifact")
        .and_then(Value::as_object)
        .and_then(|artifact| artifact.get("id"))
        .cloned()
        .and_then(|id| serde_json::from_value::<ArtifactId>(id).ok())
    {
        return format!("artifact {artifact_id} is available for follow-up reads");
    }

    summarize_json(output)
}

fn snapshot_summary(snapshot: &Value) -> Option<String> {
    if let Ok(snapshot) = serde_json::from_value::<Snapshot>(snapshot.clone()) {
        return Some(VisualObservationSummary::from_snapshot(&snapshot).describe());
    }

    let snapshot_id = snapshot.get("id").and_then(Value::as_str)?;
    let surface = snapshot
        .get("surface")
        .and_then(|surface| match surface {
            Value::String(name) => Some(name.clone()),
            Value::Object(map) => map
                .get("kind")
                .and_then(Value::as_str)
                .map(|kind| kind.to_ascii_lowercase()),
            _ => None,
        })
        .unwrap_or_else(|| "unknown".into());
    let root_count = snapshot
        .get("root_ids")
        .and_then(Value::as_array)
        .map_or(0, |items| items.len());
    let element_count = snapshot
        .get("elements")
        .and_then(Value::as_object)
        .map_or(0, |items| items.len());
    let screenshot = snapshot
        .get("image_artifact")
        .and_then(Value::as_str)
        .map(|artifact| format!(", screenshot={artifact}"))
        .unwrap_or_default();

    Some(format!(
        "snapshot {snapshot_id} on {surface} (roots={root_count}, elements={element_count}){screenshot}"
    ))
}

pub(crate) fn summarize_json(value: &Value) -> String {
    match value {
        Value::Null => "null result".into(),
        Value::Bool(flag) => format!("result={flag}"),
        Value::Number(number) => format!("result={number}"),
        Value::String(text) => truncate(text),
        Value::Array(items) => summarize_array(items),
        Value::Object(map) => summarize_object(map),
    }
}

fn planner_message(message: &AgentMessage) -> Message {
    match message {
        AgentMessage::Base { message } => compact_message(message),
        AgentMessage::Custom { kind, payload } => Message::User(UserMessage {
            content: vec![ContentBlock::Text {
                text: format!("{kind}: {}", summarize_json(payload)),
            }],
            timestamp_ms: 0,
        }),
    }
}

fn compact_message(message: &Message) -> Message {
    match message {
        Message::User(message) => Message::User(UserMessage {
            content: compact_content(&message.content),
            timestamp_ms: message.timestamp_ms,
        }),
        Message::Assistant(message) => Message::Assistant(AssistantMessage {
            content: compact_content(&message.content),
            usage: message.usage.clone(),
            stop: message.stop,
            error_message: message.error_message.clone(),
            timestamp_ms: message.timestamp_ms,
        }),
        Message::ToolResult(message) => Message::ToolResult(ToolResultMessage {
            tool_call_id: message.tool_call_id.clone(),
            tool_name: message.tool_name.clone(),
            content: compact_content(&message.content),
            is_error: message.is_error,
            timestamp_ms: message.timestamp_ms,
        }),
    }
}

fn compact_content(content: &[ContentBlock]) -> Vec<ContentBlock> {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => ContentBlock::Text {
                text: truncate_to(text, MAX_CONTEXT_TEXT_CHARS),
            },
            ContentBlock::Thinking { thinking } => ContentBlock::Thinking {
                thinking: truncate_to(thinking, MAX_CONTEXT_TEXT_CHARS),
            },
            ContentBlock::ToolCall {
                id,
                name,
                arguments_json,
            } => ContentBlock::ToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments_json: truncate_to(arguments_json, MAX_CONTEXT_TEXT_CHARS),
            },
            ContentBlock::Image { mime, data_base64 } => ContentBlock::Image {
                mime: mime.clone(),
                data_base64: data_base64.clone(),
            },
        })
        .collect()
}

fn message_char_count(message: &Message) -> usize {
    match message {
        Message::User(message) => content_char_count(&message.content),
        Message::Assistant(message) => content_char_count(&message.content),
        Message::ToolResult(message) => content_char_count(&message.content),
    }
}

fn content_char_count(content: &[ContentBlock]) -> usize {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.chars().count(),
            ContentBlock::Thinking { thinking } => thinking.chars().count(),
            ContentBlock::ToolCall {
                id,
                name,
                arguments_json,
            } => id.chars().count() + name.chars().count() + arguments_json.chars().count(),
            ContentBlock::Image { .. } => 0,
        })
        .sum()
}

fn summarize_array(items: &[Value]) -> String {
    if items.is_empty() {
        return "empty list".into();
    }

    let preview = items
        .iter()
        .take(3)
        .map(summarize_preview_value)
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if items.len() > 3 { ", ..." } else { "" };
    format!("list(len={}, items=[{}{suffix}])", items.len(), preview)
}

fn summarize_object(map: &serde_json::Map<String, Value>) -> String {
    let mut keys = map.keys().cloned().collect::<Vec<_>>();
    keys.sort();

    let preview = keys
        .iter()
        .take(4)
        .map(|key| match map.get(key).expect("sorted key should exist") {
            Value::Null => format!("{key}=null"),
            Value::Bool(flag) => format!("{key}={flag}"),
            Value::Number(number) => format!("{key}={number}"),
            Value::String(text) => format!("{key}={}", truncate(text)),
            Value::Array(items) => format!("{key}[{}]", items.len()),
            Value::Object(_) => key.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if keys.len() > 4 { ", ..." } else { "" };
    format!("result: {preview}{suffix}")
}

fn summarize_preview_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => truncate(text),
        Value::Array(items) => format!("list({})", items.len()),
        Value::Object(_) => "object".into(),
    }
}

fn truncate(text: &str) -> String {
    truncate_to(text, MAX_TEXT_SUMMARY_CHARS)
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn is_bootstrap_app_context_empty(context: &BootstrapAppContext) -> bool {
    context.prelaunched_app.is_none() && context.installed_catalog.is_none()
}

fn truncate_to(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}
