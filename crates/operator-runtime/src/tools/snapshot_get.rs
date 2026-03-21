use std::sync::Arc;

use operator_core::{OperatorError, Snapshot, SnapshotId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{tools::json_schema_for, RuntimeCore, ToolRegistration, ToolSpec};

pub(crate) fn registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "snapshot-get",
            description: "Load a previously captured snapshot from the snapshot store.",
            input_schema: json_schema_for::<SnapshotGetToolInput>(),
            output_schema: json_schema_for::<SnapshotGetToolOutput>(),
            capabilities_required: &[],
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, _ctx| Box::pin(async move { invoke(input, core).await })),
    }
}

async fn invoke(input: Value, core: Arc<RuntimeCore>) -> Result<Value, OperatorError> {
    let input = serde_json::from_value::<SnapshotGetToolInput>(input).map_err(|error| {
        OperatorError::Tool {
            tool: "snapshot-get".into(),
            message: format!("invalid input: {error}"),
        }
    })?;

    let snapshot = core
        .snapshots()
        .get(&input.snapshot_id)
        .await?
        .ok_or_else(|| OperatorError::SnapshotNotFound(input.snapshot_id.clone()))?;

    serde_json::to_value(SnapshotGetToolOutput { snapshot }).map_err(|error| OperatorError::Tool {
        tool: "snapshot-get".into(),
        message: format!("failed to serialize output: {error}"),
    })
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SnapshotGetToolInput {
    snapshot_id: SnapshotId,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct SnapshotGetToolOutput {
    snapshot: Snapshot,
}
