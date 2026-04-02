use std::sync::Arc;

use operator_core::{OperatorError, SessionId, TargetId};
use operator_runtime::{RuntimeCore, ToolRegistry, ToolSpec as RuntimeToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AgentError;

use super::{AgentToolSpec, ToolCatalogOptions};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolError {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolResult {
    pub tool_name: String,
    pub arguments: Value,
    pub output: Option<Value>,
    pub error: Option<AgentToolError>,
    pub is_error: bool,
    pub read_only: bool,
}

#[derive(Clone)]
pub struct ToolExecutor {
    core: Arc<RuntimeCore>,
    tools: ToolRegistry,
}

impl ToolExecutor {
    pub fn new(core: Arc<RuntimeCore>, tools: ToolRegistry) -> Self {
        Self { core, tools }
    }

    pub fn catalog(&self, target: &TargetId) -> Result<Vec<AgentToolSpec>, AgentError> {
        self.catalog_with_options(target, ToolCatalogOptions::default())
    }

    pub fn catalog_with_options(
        &self,
        target: &TargetId,
        options: ToolCatalogOptions,
    ) -> Result<Vec<AgentToolSpec>, AgentError> {
        let (_, driver) = self.core.resolve_driver(target)?;
        let capabilities = driver.capabilities();
        let allow_side_effects = self.core.config().allow_side_effects;

        Ok(self
            .tools
            .specs()
            .into_iter()
            .filter(|spec| {
                (allow_side_effects || !spec.has_side_effects)
                    && spec
                        .capabilities_required
                        .iter()
                        .all(|capability| capabilities.supports(capability))
            })
            .map(AgentToolSpec::from)
            .map(|spec| spec.with_catalog_options(options))
            .collect())
    }

    pub async fn call(
        &self,
        session_id: &SessionId,
        target: &TargetId,
        name: &str,
        arguments: Value,
        timeout_ms: Option<u64>,
    ) -> Result<AgentToolResult, AgentError> {
        let read_only = self
            .find_spec(name)
            .map(|spec| !spec.has_side_effects)
            .unwrap_or(false);
        let mut normalized_args = match arguments.clone() {
            Value::Object(map) => map,
            _ => {
                return Err(AgentError::Planner(
                    "tool arguments must be encoded as a JSON object".into(),
                ));
            }
        };
        normalize_agent_args(name, &mut normalized_args);
        let input = merge_exec_context(
            Value::Object(normalized_args),
            session_id,
            target,
            timeout_ms,
        )?;

        match self.tools.invoke(name, input).await {
            Ok(output) => Ok(AgentToolResult {
                tool_name: name.to_string(),
                arguments,
                output: Some(output),
                error: None,
                is_error: false,
                read_only,
            }),
            Err(error) => Ok(AgentToolResult {
                tool_name: name.to_string(),
                arguments,
                output: None,
                error: Some(AgentToolError {
                    kind: error_kind(&error).to_string(),
                    message: error.to_string(),
                }),
                is_error: true,
                read_only,
            }),
        }
    }

    fn find_spec(&self, name: &str) -> Option<RuntimeToolSpec> {
        self.tools
            .specs()
            .into_iter()
            .find(|spec| spec.name == name)
    }
}

/// Translate simplified agent-facing arguments to full runtime format before dispatch.
///
/// - Locator fields: `{"element":"e37"}` / `{"text":"..."}` / `{"x":N,"y":N}` →
///   `SnapshotElement` / `SnapshotText` / `Coords` variants (all use `"latest"` snapshot).
/// - Observe tool: `{"elements":bool}` → `{"surface":{"kind":"Frontmost"},
///   "include_screenshot":false,"include_elements":bool}`.
fn normalize_agent_args(name: &str, args: &mut serde_json::Map<String, Value>) {
    if let Some(locator) = args.get("locator").cloned() {
        if let Some(normalized) = normalize_agent_locator(&locator) {
            args.insert("locator".into(), normalized);
        }
    }

    if name == "observe" {
        normalize_agent_observe(args);
    }
}

fn normalize_agent_locator(locator: &Value) -> Option<Value> {
    let obj = locator.as_object()?;

    // Already in runtime format — skip normalization.
    if obj.contains_key("SnapshotElement")
        || obj.contains_key("SnapshotText")
        || obj.contains_key("SnapshotPixelCoords")
        || obj.contains_key("SnapshotCoords")
        || obj.contains_key("SnapshotNormalizedCoords")
        || obj.contains_key("Text")
        || obj.contains_key("Role")
        || obj.contains_key("Coords")
    {
        return None;
    }

    // {"element": "e37"} → SnapshotElement with "latest" sentinel.
    if let Some(element) = obj.get("element").and_then(Value::as_str) {
        return Some(serde_json::json!({
            "SnapshotElement": {
                "snapshot": "latest",
                "element": element
            }
        }));
    }

    // {"text": "foo"} → SnapshotText with "latest" sentinel.
    if let Some(text) = obj.get("text").and_then(Value::as_str) {
        return Some(serde_json::json!({
            "SnapshotText": {
                "snapshot": "latest",
                "text": text
            }
        }));
    }

    // {"x": N, "y": N} → Coords.
    if let (Some(x), Some(y)) = (obj.get("x"), obj.get("y")) {
        return Some(serde_json::json!({
            "Coords": { "x": x, "y": y }
        }));
    }

    None
}

fn normalize_agent_observe(args: &mut serde_json::Map<String, Value>) {
    let elements = args
        .remove("elements")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !args.contains_key("surface") {
        args.insert("surface".into(), serde_json::json!({ "kind": "Frontmost" }));
    }
    args.insert("include_elements".into(), Value::Bool(elements));
    args.entry("include_screenshot".to_string())
        .or_insert(Value::Bool(false));
}

fn merge_exec_context(
    arguments: Value,
    session_id: &SessionId,
    target: &TargetId,
    timeout_ms: Option<u64>,
) -> Result<Value, AgentError> {
    let mut arguments = match arguments {
        Value::Object(map) => map,
        _ => {
            return Err(AgentError::Planner(
                "tool arguments must be encoded as a JSON object".into(),
            ));
        }
    };

    arguments.insert("target".into(), Value::String(target.to_string()));
    arguments.insert("session_id".into(), Value::String(session_id.to_string()));
    if let Some(timeout_ms) = timeout_ms {
        arguments.insert("timeout_ms".into(), Value::Number(timeout_ms.into()));
    }

    Ok(Value::Object(arguments))
}

fn error_kind(error: &OperatorError) -> &'static str {
    match error {
        OperatorError::CapabilityNotSupported(_) => "capability_not_supported",
        OperatorError::TargetNotFound(_) => "target_not_found",
        OperatorError::DriverUnavailable { .. } => "driver_unavailable",
        OperatorError::TargetBusy => "target_busy",
        OperatorError::Timeout { .. } => "timeout",
        OperatorError::ElementNotFound(_) => "element_not_found",
        OperatorError::SnapshotNotFound(_) => "snapshot_not_found",
        OperatorError::PermissionDenied(_) => "permission_denied",
        OperatorError::Platform(_) => "platform",
        OperatorError::Tool { .. } => "tool",
        OperatorError::Model(_) => "model",
        OperatorError::Io(_) => "io",
        OperatorError::Serialization(_) => "serialization",
    }
}
