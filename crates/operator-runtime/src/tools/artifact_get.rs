use std::sync::Arc;

use operator_core::{ArtifactId, OperatorError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{tools::json_schema_for, RuntimeCore, ToolRegistration, ToolSpec};

pub(crate) fn registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "artifact-get",
            description: "Resolve a persisted capture artifact to its runtime-managed file path.",
            input_schema: json_schema_for::<ArtifactGetToolInput>(),
            output_schema: json_schema_for::<ArtifactGetToolOutput>(),
            capabilities_required: &[],
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, _ctx| Box::pin(async move { invoke(input, core).await })),
    }
}

async fn invoke(input: Value, core: Arc<RuntimeCore>) -> Result<Value, OperatorError> {
    let input = serde_json::from_value::<ArtifactGetToolInput>(input).map_err(|error| {
        OperatorError::Tool {
            tool: "artifact-get".into(),
            message: format!("invalid input: {error}"),
        }
    })?;
    input
        .artifact_id
        .as_file_name()
        .map_err(|_| OperatorError::Tool {
            tool: "artifact-get".into(),
            message: format!("invalid artifact id: {}", input.artifact_id),
        })?;

    let path = core
        .artifacts()
        .resolve_artifact(&input.artifact_id)
        .await
        .map_err(|error| OperatorError::Tool {
            tool: "artifact-get".into(),
            message: error.to_string(),
        })?;
    if !path.exists() {
        return Err(OperatorError::Tool {
            tool: "artifact-get".into(),
            message: format!("artifact not found: {}", input.artifact_id),
        });
    }

    serde_json::to_value(ArtifactGetToolOutput {
        artifact: ArtifactOutput {
            id: input.artifact_id,
            path: path.to_string_lossy().to_string(),
        },
    })
    .map_err(|error| OperatorError::Tool {
        tool: "artifact-get".into(),
        message: format!("failed to serialize output: {error}"),
    })
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ArtifactGetToolInput {
    artifact_id: ArtifactId,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ArtifactGetToolOutput {
    artifact: ArtifactOutput,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ArtifactOutput {
    id: ArtifactId,
    path: String,
}
