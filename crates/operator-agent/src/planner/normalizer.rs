use operator_core::{Point, SnapshotId};
use serde_json::Value;

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
        let arguments = normalize_arguments(arguments, model.coordinate_policy, snapshot)?;

        Ok(AgentDecision::CallTool {
            name,
            arguments,
            summary,
            thought,
        })
    }
}

fn normalize_arguments(
    arguments: Value,
    policy: CoordinatePolicy,
    snapshot: Option<SnapshotId>,
) -> Result<Value, AgentError> {
    if matches!(policy, CoordinatePolicy::ScreenAbsolutePixels) {
        return Ok(arguments);
    }

    let Value::Object(mut object) = arguments else {
        return Ok(arguments);
    };

    for key in ["locator", "from", "to"] {
        if let Some(value) = object.remove(key) {
            object.insert(
                key.to_string(),
                normalize_locator_value(value, policy, snapshot.clone())?,
            );
        }
    }

    Ok(Value::Object(object))
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
