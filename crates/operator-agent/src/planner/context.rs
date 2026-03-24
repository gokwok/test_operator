use std::sync::Arc;

use operator_core::{
    ArtifactId, Capability, OperatorError, Snapshot, SnapshotId, SurfaceKind, TargetId,
};
use operator_runtime::RuntimeCore;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{session::AgentSessionState, AgentError};

const DEFAULT_RECENT_TOOL_RESULTS: usize = 5;
const MAX_TEXT_SUMMARY_CHARS: usize = 120;

#[derive(Clone)]
pub struct ContextAssembler {
    core: Arc<RuntimeCore>,
    recent_tool_limit: usize,
}

impl ContextAssembler {
    pub fn new(core: Arc<RuntimeCore>) -> Self {
        Self {
            core,
            recent_tool_limit: DEFAULT_RECENT_TOOL_RESULTS,
        }
    }

    pub fn with_recent_tool_limit(mut self, recent_tool_limit: usize) -> Self {
        self.recent_tool_limit = recent_tool_limit;
        self
    }

    pub async fn assemble(&self, state: &AgentSessionState) -> Result<PlannerContext, AgentError> {
        Ok(PlannerContext {
            target: self.summarize_target(&state.target)?,
            recent_tool_results: self.summarize_tool_results(state),
            latest_snapshot: self.load_latest_snapshot(state).await?,
            previous_snapshot_visual: state.previous_snapshot_visual.clone(),
            notes: state.notes.clone(),
            ui_state_stale: state.ui_state_stale,
        })
    }

    fn summarize_target(&self, target: &TargetId) -> Result<TargetSummary, AgentError> {
        let (descriptor, driver) = self.core.resolve_driver(target)?;
        let mut capabilities = driver
            .capabilities()
            .iter()
            .map(capability_name)
            .collect::<Vec<_>>();
        capabilities.sort();

        Ok(TargetSummary {
            id: descriptor.id,
            platform: descriptor.platform,
            capabilities,
        })
    }

    async fn load_latest_snapshot(
        &self,
        state: &AgentSessionState,
    ) -> Result<Option<SnapshotSummary>, AgentError> {
        let Some(snapshot_id) = &state.latest_snapshot else {
            return Ok(None);
        };

        let snapshot = self
            .core
            .snapshots()
            .get(snapshot_id)
            .await?
            .ok_or_else(|| {
                AgentError::Runtime(OperatorError::SnapshotNotFound(snapshot_id.clone()))
            })?;
        Ok(Some(SnapshotSummary::from_snapshot(&snapshot)))
    }

    fn summarize_tool_results(&self, state: &AgentSessionState) -> Vec<ToolResultSummary> {
        let trace = &state.tool_trace;
        let start = trace.len().saturating_sub(self.recent_tool_limit);

        trace[start..]
            .iter()
            .map(|entry| ToolResultSummary {
                turn_index: entry.turn_index,
                step_index: entry.step_index,
                tool_name: entry.result.tool_name.clone(),
                is_error: entry.result.is_error,
                read_only: entry.result.read_only,
                summary: summarize_tool_output(&entry.result),
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerContext {
    pub target: TargetSummary,
    pub recent_tool_results: Vec<ToolResultSummary>,
    pub latest_snapshot: Option<SnapshotSummary>,
    pub previous_snapshot_visual: Option<ArtifactId>,
    pub notes: Vec<String>,
    pub ui_state_stale: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSummary {
    pub id: TargetId,
    pub platform: String,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub id: SnapshotId,
    pub surface: String,
    pub root_element_count: usize,
    pub element_count: usize,
    pub screenshot_artifact: Option<ArtifactId>,
}

impl SnapshotSummary {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            surface: surface_name(&snapshot.surface.kind),
            root_element_count: snapshot.root_ids.len(),
            element_count: snapshot.elements.len(),
            screenshot_artifact: snapshot.image_artifact.clone(),
        }
    }

    fn describe(&self) -> String {
        let screenshot = self
            .screenshot_artifact
            .as_ref()
            .map(|artifact| format!(", screenshot={artifact}"))
            .unwrap_or_default();
        format!(
            "snapshot {} on {} (roots={}, elements={}){}",
            self.id, self.surface, self.root_element_count, self.element_count, screenshot
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultSummary {
    pub turn_index: u32,
    pub step_index: u32,
    pub tool_name: String,
    pub is_error: bool,
    pub read_only: bool,
    pub summary: String,
}

fn summarize_tool_output(result: &crate::tools::AgentToolResult) -> String {
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

    if let Some(snapshot) = snapshot_from_output(output) {
        return SnapshotSummary::from_snapshot(&snapshot).describe();
    }

    if let Some(artifact_id) = artifact_id_from_output(output) {
        return format!("artifact {} is available for follow-up reads", artifact_id);
    }

    summarize_json(output)
}

fn snapshot_from_output(output: &Value) -> Option<Snapshot> {
    output
        .get("snapshot")
        .cloned()
        .and_then(|snapshot| serde_json::from_value(snapshot).ok())
}

fn artifact_id_from_output(output: &Value) -> Option<ArtifactId> {
    output
        .get("artifact")
        .and_then(Value::as_object)
        .and_then(|artifact| artifact.get("id"))
        .cloned()
        .and_then(|id| serde_json::from_value(id).ok())
}

fn summarize_json(value: &Value) -> String {
    match value {
        Value::Null => "null result".into(),
        Value::Bool(flag) => format!("result={flag}"),
        Value::Number(number) => format!("result={number}"),
        Value::String(text) => truncate(text),
        Value::Array(items) => summarize_array(items),
        Value::Object(map) => summarize_object(map),
    }
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

fn summarize_object(map: &Map<String, Value>) -> String {
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
        Value::Object(map) => {
            if let Some(name) = map.get("name").and_then(Value::as_str) {
                return format!("name={}", truncate(name));
            }
            if let Some(title) = map.get("title").and_then(Value::as_str) {
                return format!("title={}", truncate(title));
            }

            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            format!(
                "object(keys=[{}])",
                keys.into_iter().take(3).collect::<Vec<_>>().join(", ")
            )
        }
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_TEXT_SUMMARY_CHARS {
        return text.to_string();
    }

    let truncated = text
        .chars()
        .take(MAX_TEXT_SUMMARY_CHARS)
        .collect::<String>();
    format!("{truncated}...")
}

fn capability_name(capability: &Capability) -> String {
    match capability {
        Capability::Capture => "capture".into(),
        Capability::InspectTree => "inspect_tree".into(),
        Capability::InspectText => "inspect_text".into(),
        Capability::PointerInput => "pointer_input".into(),
        Capability::KeyboardInput => "keyboard_input".into(),
        Capability::WindowManagement => "window_management".into(),
        Capability::AppLifecycle => "app_lifecycle".into(),
        Capability::Clipboard => "clipboard".into(),
        Capability::Permissions => "permissions".into(),
        Capability::DeviceInfo => "device_info".into(),
        Capability::Extension(id) => format!("{}:{}", id.namespace, id.name),
    }
}

fn surface_name(surface: &SurfaceKind) -> String {
    match surface {
        SurfaceKind::Fullscreen {
            display_id: Some(display_id),
        } => {
            format!("fullscreen(display={display_id})")
        }
        SurfaceKind::Fullscreen { display_id: None } => "fullscreen".into(),
        SurfaceKind::Frontmost => "frontmost".into(),
        SurfaceKind::Window { id } => format!("window({id})"),
        SurfaceKind::Region { rect } => {
            format!(
                "region({}, {}, {}, {})",
                rect.x, rect.y, rect.width, rect.height
            )
        }
    }
}
