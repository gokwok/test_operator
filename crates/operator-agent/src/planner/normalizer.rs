use operator_core::{Point, SnapshotId};
use serde_json::{Map, Value};

use crate::{
    model::{CoordinatePolicy, ModelConfig},
    session::AgentSessionState,
    AgentError,
};

use super::AgentDecision;

#[derive(Clone, Debug, Default)]
pub struct DecisionNormalizer;

impl DecisionNormalizer {
    pub fn new() -> Self {
        Self
    }

    pub fn normalize(
        &self,
        decision: AgentDecision,
        model: &ModelConfig,
        state: &AgentSessionState,
        include_elements: bool,
    ) -> Result<AgentDecision, AgentError> {
        let AgentDecision::CallTool {
            name,
            arguments,
            summary,
            thought,
        } = decision
        else {
            return Ok(decision);
        };

        let snapshot = state
            .current_observation()
            .map(|observation| observation.snapshot_id.clone());
        let frontmost_observation = state
            .current_observation()
            .is_some_and(|observation| observation.surface == "frontmost");
        let arguments = normalize_arguments(
            &name,
            arguments,
            model.coordinate_policy,
            snapshot,
            include_elements,
            frontmost_observation,
        )?;

        Ok(AgentDecision::CallTool {
            name,
            arguments,
            summary,
            thought,
        })
    }
}

fn normalize_arguments(
    tool_name: &str,
    arguments: Value,
    policy: CoordinatePolicy,
    snapshot: Option<SnapshotId>,
    include_elements: bool,
    frontmost_observation: bool,
) -> Result<Value, AgentError> {
    if matches!(policy, CoordinatePolicy::ScreenAbsolutePixels) {
        return normalize_tool_specific_arguments(
            tool_name,
            arguments,
            include_elements,
            frontmost_observation,
        );
    }

    let Value::Object(mut object) = arguments else {
        return normalize_tool_specific_arguments(
            tool_name,
            arguments,
            include_elements,
            frontmost_observation,
        );
    };

    for key in ["locator", "from", "to"] {
        if let Some(value) = object.remove(key) {
            object.insert(
                key.to_string(),
                normalize_locator_value(value, policy, snapshot.clone())?,
            );
        }
    }

    normalize_tool_specific_arguments(
        tool_name,
        Value::Object(object),
        include_elements,
        frontmost_observation,
    )
}

fn normalize_tool_specific_arguments(
    tool_name: &str,
    arguments: Value,
    include_elements: bool,
    frontmost_observation: bool,
) -> Result<Value, AgentError> {
    let Value::Object(mut object) = arguments else {
        return Ok(arguments);
    };

    if tool_name == "observe" && !include_elements {
        object.insert("include_elements".into(), Value::Bool(false));
    }

    if frontmost_observation && supports_frontmost_direct_action(tool_name) {
        strip_frontmost_app_selector(&mut object);
        if !object.contains_key("target_selector") {
            object.remove("verifications");
        }
    }

    Ok(Value::Object(object))
}

fn supports_frontmost_direct_action(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "click" | "type" | "press" | "hotkey" | "scroll" | "move" | "drag" | "swipe"
    )
}

fn strip_frontmost_app_selector(object: &mut Map<String, Value>) {
    let should_strip_null = object.get("target_selector").is_some_and(Value::is_null);
    let should_strip_app = object
        .get("target_selector")
        .and_then(Value::as_object)
        .is_some_and(|selector| selector.len() == 1 && selector.contains_key("App"));

    if should_strip_null || should_strip_app {
        object.remove("target_selector");
    }
}

fn normalize_locator_value(
    value: Value,
    policy: CoordinatePolicy,
    snapshot: Option<SnapshotId>,
) -> Result<Value, AgentError> {
    let Value::Object(mut object) = value else {
        return Ok(value);
    };

    if let Some(coords) = object.remove("Coords") {
        return wrap_point_for_policy(parse_point(&coords)?, policy, snapshot);
    }

    if let Some(coords) = object.remove("SnapshotPixelCoords") {
        return wrap_point_for_policy(parse_snapshot_point(&coords)?.1, policy, snapshot);
    }

    if let Some(coords) = object.remove("SnapshotCoords") {
        return wrap_point_for_policy(parse_snapshot_point(&coords)?.1, policy, snapshot);
    }

    if let Some(coords) = object.remove("SnapshotNormalizedCoords") {
        return wrap_point_for_policy(
            parse_snapshot_normalized_point(&coords)?.1,
            policy,
            snapshot,
        );
    }

    Ok(Value::Object(object))
}

fn parse_point(value: &Value) -> Result<Point, AgentError> {
    let object = value.as_object().ok_or_else(|| {
        AgentError::Planner("coordinate locator must be encoded as an object".into())
    })?;
    let x = object
        .get("x")
        .and_then(Value::as_f64)
        .ok_or_else(|| AgentError::Planner("coordinate locator is missing numeric `x`".into()))?;
    let y = object
        .get("y")
        .and_then(Value::as_f64)
        .ok_or_else(|| AgentError::Planner("coordinate locator is missing numeric `y`".into()))?;
    Ok(Point { x, y })
}

fn parse_snapshot_point(value: &Value) -> Result<(SnapshotId, Point), AgentError> {
    let object = value.as_object().ok_or_else(|| {
        AgentError::Planner("snapshot coordinate locator must be encoded as an object".into())
    })?;
    let snapshot = object
        .get("snapshot")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AgentError::Planner("snapshot coordinate locator is missing string `snapshot`".into())
        })?;
    let point = object
        .get("point")
        .ok_or_else(|| AgentError::Planner("snapshot coordinate locator is missing `point`".into()))
        .and_then(parse_point)?;
    Ok((snapshot.into(), point))
}

fn parse_snapshot_normalized_point(value: &Value) -> Result<(SnapshotId, Point, f64), AgentError> {
    let object = value.as_object().ok_or_else(|| {
        AgentError::Planner(
            "snapshot normalized coordinate locator must be encoded as an object".into(),
        )
    })?;
    let (snapshot, point) = parse_snapshot_point(value)?;
    let basis = object.get("basis").and_then(Value::as_f64).ok_or_else(|| {
        AgentError::Planner(
            "snapshot normalized coordinate locator is missing numeric `basis`".into(),
        )
    })?;
    Ok((snapshot, point, basis))
}

fn wrap_point_for_policy(
    point: Point,
    policy: CoordinatePolicy,
    snapshot: Option<SnapshotId>,
) -> Result<Value, AgentError> {
    let snapshot = snapshot.ok_or_else(|| {
        AgentError::Planner(
            "coordinate-based planner output requires a current observation snapshot".into(),
        )
    })?;

    let normalized = match policy {
        CoordinatePolicy::ScreenAbsolutePixels => serde_json::json!({
            "Coords": point,
        }),
        CoordinatePolicy::SurfaceImagePixels => serde_json::json!({
            "SnapshotPixelCoords": {
                "snapshot": snapshot,
                "point": point,
            }
        }),
        CoordinatePolicy::SurfaceAbsolutePixels => serde_json::json!({
            "SnapshotCoords": {
                "snapshot": snapshot,
                "point": point,
            }
        }),
        CoordinatePolicy::SurfaceNormalized1000 => serde_json::json!({
            "SnapshotNormalizedCoords": {
                "snapshot": snapshot,
                "point": point,
                "basis": 1000.0,
            }
        }),
    };

    Ok(normalized)
}
