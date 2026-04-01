use std::sync::Arc;

use operator_core::{ArtifactId, Capability, TargetId};
use operator_runtime::RuntimeCore;
use serde::{Deserialize, Serialize};

use crate::{
    session::{
        summarize_tool_result, AgentSessionState, BootstrapAppContext, VisualObservationSummary,
    },
    AgentError,
};

const DEFAULT_RECENT_TOOL_RESULTS: usize = 5;

#[derive(Clone)]
pub struct LoopStateContextManager {
    core: Arc<RuntimeCore>,
    recent_tool_limit: usize,
}

impl LoopStateContextManager {
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

    pub fn assemble(&self, state: &AgentSessionState) -> Result<PlannerContext, AgentError> {
        Ok(PlannerContext {
            target: self.summarize_target(&state.target)?,
            include_elements: state.include_elements(),
            recent_tool_results: self.summarize_tool_results(state),
            current_observation: state.current_observation().cloned(),
            current_visual_artifact: state.current_visual().cloned(),
            previous_visual_artifact: state.previous_visual().cloned(),
            notes: state.notes.clone(),
            app_bootstrap: (!state.app_bootstrap.eq(&BootstrapAppContext::default()))
                .then(|| state.app_bootstrap.clone()),
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
                summary: summarize_tool_result(&entry.result),
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannerContext {
    pub target: TargetSummary,
    pub include_elements: bool,
    pub recent_tool_results: Vec<ToolResultSummary>,
    pub current_observation: Option<VisualObservationSummary>,
    pub current_visual_artifact: Option<ArtifactId>,
    pub previous_visual_artifact: Option<ArtifactId>,
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_bootstrap: Option<BootstrapAppContext>,
    pub ui_state_stale: bool,
}

impl PlannerContext {
    pub fn visual_references(&self) -> Vec<PlannerVisualReference> {
        let mut visuals = Vec::new();
        if let Some(artifact_id) = self.previous_visual_artifact.clone() {
            visuals.push(PlannerVisualReference {
                slot: PlannerVisualSlot::Previous,
                artifact_id,
            });
        }
        if let Some(artifact_id) = self.current_visual_artifact.clone() {
            visuals.push(PlannerVisualReference {
                slot: PlannerVisualSlot::Current,
                artifact_id,
            });
        }
        visuals
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlannerVisualSlot {
    Previous,
    Current,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannerVisualReference {
    pub slot: PlannerVisualSlot,
    pub artifact_id: ArtifactId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSummary {
    pub id: TargetId,
    pub platform: String,
    pub capabilities: Vec<String>,
}

impl TargetSummary {
    pub fn capabilities_text(&self) -> String {
        if self.capabilities.is_empty() {
            "none".into()
        } else {
            self.capabilities.join(", ")
        }
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

impl ToolResultSummary {
    pub fn render_line(&self) -> String {
        let outcome = if self.is_error {
            "error"
        } else if self.read_only {
            "read-only"
        } else {
            "side-effect"
        };

        format!(
            "turn {} step {} {} [{}]: {}",
            self.turn_index, self.step_index, self.tool_name, outcome, self.summary
        )
    }
}

fn capability_name(capability: &Capability) -> String {
    match capability {
        Capability::Capture => "capture".into(),
        Capability::InspectTree => "inspect_tree".into(),
        Capability::InspectText => "inspect_text".into(),
        Capability::PointerInput => "pointer_input".into(),
        Capability::KeyboardInput => "keyboard_input".into(),
        Capability::WindowQuery => "window_query".into(),
        Capability::WindowManagement => "window_management".into(),
        Capability::AppLifecycle => "app_lifecycle".into(),
        Capability::Clipboard => "clipboard".into(),
        Capability::Permissions => "permissions".into(),
        Capability::DeviceInfo => "device_info".into(),
        Capability::Extension(id) => format!("{}:{}", id.namespace, id.name),
    }
}
