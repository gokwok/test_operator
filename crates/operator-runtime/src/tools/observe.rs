use std::sync::Arc;

use operator_core::{ObserveRequest, OperatorError, Snapshot, Surface};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    tools::{json_schema_for, ToolExecInput},
    RuntimeCore, ToolRegistration, ToolSpec,
};

pub(crate) fn registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "observe",
            description: "Capture a surface and persist the resulting snapshot.",
            input_schema: json_schema_for::<ObserveToolInput>(),
            output_schema: json_schema_for::<ObserveToolOutput>(),
            capabilities_required: &[],
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { invoke(input, core, ctx).await })
        }),
    }
}

async fn invoke(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input =
        serde_json::from_value::<ObserveToolInput>(input).map_err(|error| OperatorError::Tool {
            tool: "observe".into(),
            message: format!("invalid input: {error}"),
        })?;

    let observed = core
        .observe(
            ObserveRequest {
                surface: input.surface,
                include_screenshot: input.include_screenshot,
                include_elements: input.include_elements,
            },
            ctx,
        )
        .await?;

    serde_json::to_value(ObserveToolOutput {
        snapshot: observed.snapshot,
    })
    .map_err(|error| OperatorError::Tool {
        tool: "observe".into(),
        message: format!("failed to serialize output: {error}"),
    })
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ObserveToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    surface: Surface,
    #[serde(default)]
    include_screenshot: bool,
    #[serde(default)]
    include_elements: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ObserveToolOutput {
    snapshot: Snapshot,
}
