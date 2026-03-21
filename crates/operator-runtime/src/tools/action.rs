use std::sync::Arc;

use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, Locator, MouseButton, OperatorError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    tools::{json_schema_for, ToolExecInput},
    RuntimeCore, ToolRegistration, ToolSpec,
};

const CLICK_CAPABILITIES: &[Capability] = &[Capability::PointerInput];
const TYPE_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const LAUNCH_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];

pub(crate) fn registrations() -> Vec<ToolRegistration> {
    vec![
        click_registration(),
        type_registration(),
        launch_app_registration(),
    ]
}

fn click_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "click",
            description: "Perform a pointer click, optionally scoped by a locator.",
            input_schema: json_schema_for::<ClickToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: CLICK_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { click(input, core, ctx).await })
        }),
    }
}

fn type_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "type",
            description: "Type text, optionally into a locator-resolved target.",
            input_schema: json_schema_for::<TypeToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: TYPE_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { r#type(input, core, ctx).await })
        }),
    }
}

fn launch_app_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "launch-app",
            description: "Launch an app by bundle identifier or app name.",
            input_schema: json_schema_for::<LaunchAppToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: LAUNCH_APP_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { launch_app(input, core, ctx).await })
        }),
    }
}

async fn click(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<ClickToolInput>("click", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Click {
                    button: input.button,
                },
                locator: input.locator,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn r#type(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<TypeToolInput>("type", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Type { text: input.text },
                locator: input.locator,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn launch_app(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<LaunchAppToolInput>("launch-app", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::LaunchApp {
                    bundle_id_or_name: input.bundle_id_or_name,
                },
                locator: None,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

fn parse_input<T: for<'de> Deserialize<'de>>(tool: &str, input: Value) -> Result<T, OperatorError> {
    serde_json::from_value(input).map_err(|error| OperatorError::Tool {
        tool: tool.to_string(),
        message: format!("invalid input: {error}"),
    })
}

fn serialize_output(outcome: ActionOutcome) -> Result<Value, OperatorError> {
    serde_json::to_value(ActionToolOutput { outcome }).map_err(|error| OperatorError::Tool {
        tool: "action".into(),
        message: format!("failed to serialize output: {error}"),
    })
}

fn default_button() -> MouseButton {
    MouseButton::Left
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ClickToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    #[serde(default = "default_button")]
    button: MouseButton,
    locator: Option<Locator>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct TypeToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    text: String,
    locator: Option<Locator>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct LaunchAppToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    bundle_id_or_name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ActionToolOutput {
    outcome: ActionOutcome,
}
