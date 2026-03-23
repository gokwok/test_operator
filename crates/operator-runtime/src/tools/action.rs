use std::{num::NonZeroU32, sync::Arc};

use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, ClickMode, DragMotion, Locator,
    OperatorError, WindowId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    tools::{json_schema_for, ToolExecInput},
    RuntimeCore, ToolRegistration, ToolSpec,
};

const CLICK_CAPABILITIES: &[Capability] = &[Capability::PointerInput];
const MOVE_CAPABILITIES: &[Capability] = &[Capability::PointerInput];
const DRAG_CAPABILITIES: &[Capability] = &[Capability::PointerInput];
const SCROLL_CAPABILITIES: &[Capability] = &[Capability::PointerInput];
const TYPE_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const HOTKEY_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const PRESS_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const LAUNCH_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const FOCUS_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];

pub(crate) fn registrations() -> Vec<ToolRegistration> {
    vec![
        click_registration(),
        move_registration(),
        drag_registration(),
        scroll_registration(),
        type_registration(),
        hotkey_registration(),
        press_registration(),
        launch_app_registration(),
        focus_window_registration(),
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

fn move_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "move",
            description: "Move the cursor to a locator-resolved target without clicking.",
            input_schema: json_schema_for::<MoveToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: MOVE_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { move_cursor(input, core, ctx).await })
        }),
    }
}

fn drag_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "drag",
            description: "Drag from one locator to another locator.",
            input_schema: json_schema_for::<DragToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: DRAG_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| Box::pin(async move { drag(input, core, ctx).await })),
    }
}

fn scroll_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "scroll",
            description:
                "Scroll by horizontal and vertical wheel deltas, optionally scoped by a locator.",
            input_schema: json_schema_for::<ScrollToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: SCROLL_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { scroll(input, core, ctx).await })
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

fn hotkey_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "hotkey",
            description: "Send a modifier-aware key chord such as command-k.",
            input_schema: json_schema_for::<HotkeyToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: HOTKEY_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { hotkey(input, core, ctx).await })
        }),
    }
}

fn press_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "press",
            description: "Press a single special key one or more times.",
            input_schema: json_schema_for::<PressToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: PRESS_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { press(input, core, ctx).await })
        }),
    }
}

fn focus_window_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "focus-window",
            description: "Focus a specific window by id.",
            input_schema: json_schema_for::<FocusWindowToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: FOCUS_WINDOW_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { focus_window(input, core, ctx).await })
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
                action: Action::Click { mode: input.mode },
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

async fn move_cursor(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<MoveToolInput>("move", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Move,
                locator: Some(input.locator),
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn scroll(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<ScrollToolInput>("scroll", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Scroll {
                    delta_x: input.delta_x,
                    delta_y: input.delta_y,
                },
                locator: input.locator,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn drag(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<DragToolInput>("drag", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Drag {
                    from: input.from,
                    to: input.to,
                    motion: input.motion,
                },
                locator: None,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn hotkey(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<HotkeyToolInput>("hotkey", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Hotkey { keys: input.keys },
                locator: None,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn press(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<PressToolInput>("press", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::Press {
                    key: input.key,
                    count: input.count,
                },
                locator: None,
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

async fn focus_window(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<FocusWindowToolInput>("focus-window", input)?;
    let outcome = core
        .act(
            ActionRequest {
                action: Action::FocusWindow {
                    id: input.window_id,
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

fn default_click_mode() -> ClickMode {
    ClickMode::Left
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ClickToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    #[serde(default = "default_click_mode", alias = "button")]
    mode: ClickMode,
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
struct MoveToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    locator: Locator,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct HotkeyToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    keys: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct PressToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    key: String,
    #[serde(default = "default_press_count")]
    count: NonZeroU32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct DragToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    from: Locator,
    to: Locator,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    motion: DragMotion,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ScrollToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    delta_x: f64,
    delta_y: f64,
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

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct FocusWindowToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    window_id: WindowId,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ActionToolOutput {
    outcome: ActionOutcome,
}

fn default_press_count() -> NonZeroU32 {
    NonZeroU32::MIN
}
