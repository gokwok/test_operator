use std::{num::NonZeroU32, sync::Arc};

use operator_core::{
    Action, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionTargetSelector,
    ActionVerification, Capability, ClickMode, DragMotion, Locator, OperatorError, Rect,
    TypeTrailingKey, WindowId,
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
const SWIPE_CAPABILITIES: &[Capability] = &[Capability::PointerInput];
const TYPE_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const HOTKEY_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const PRESS_CAPABILITIES: &[Capability] = &[Capability::KeyboardInput];
const LAUNCH_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const CLOSE_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const MINIMIZE_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const MAXIMIZE_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const MOVE_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const RESIZE_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const SET_WINDOW_BOUNDS_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const SWITCH_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const QUIT_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const RELAUNCH_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const HIDE_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const UNHIDE_APP_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const FOCUS_WINDOW_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];

pub(crate) fn registrations() -> Vec<ToolRegistration> {
    vec![
        click_registration(),
        move_registration(),
        drag_registration(),
        swipe_registration(),
        scroll_registration(),
        type_registration(),
        hotkey_registration(),
        press_registration(),
        launch_app_registration(),
        close_window_registration(),
        minimize_window_registration(),
        maximize_window_registration(),
        move_window_registration(),
        resize_window_registration(),
        set_window_bounds_registration(),
        switch_app_registration(),
        quit_app_registration(),
        relaunch_app_registration(),
        hide_app_registration(),
        unhide_app_registration(),
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

fn swipe_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "swipe",
            description:
                "Perform gesture-style pointer motion between two locator-resolved points.",
            input_schema: json_schema_for::<SwipeToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: SWIPE_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { swipe(input, core, ctx).await })
        }),
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

fn close_window_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "close-window",
            description: "Close a target window using a shared selector and focus policy.",
            input_schema: json_schema_for::<WindowTargetToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: CLOSE_WINDOW_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { close_window(input, core, ctx).await })
        }),
    }
}

fn minimize_window_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "minimize-window",
            description: "Minimize a target window using a shared selector and focus policy.",
            input_schema: json_schema_for::<MinimizeWindowToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: MINIMIZE_WINDOW_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { minimize_window(input, core, ctx).await })
        }),
    }
}

fn maximize_window_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "maximize-window",
            description: "Maximize a target window using a shared selector and focus policy.",
            input_schema: json_schema_for::<WindowTargetToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: MAXIMIZE_WINDOW_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { maximize_window(input, core, ctx).await })
        }),
    }
}

fn move_window_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "move-window",
            description:
                "Move a target window to explicit top-left coordinates using a shared selector.",
            input_schema: json_schema_for::<MoveWindowToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: MOVE_WINDOW_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { move_window(input, core, ctx).await })
        }),
    }
}

fn resize_window_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "resize-window",
            description: "Resize a target window to explicit width and height.",
            input_schema: json_schema_for::<ResizeWindowToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: RESIZE_WINDOW_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { resize_window(input, core, ctx).await })
        }),
    }
}

fn set_window_bounds_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "set-window-bounds",
            description: "Set a target window to explicit bounds using a shared selector.",
            input_schema: json_schema_for::<SetWindowBoundsToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: SET_WINDOW_BOUNDS_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { set_window_bounds(input, core, ctx).await })
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

fn switch_app_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "switch-app",
            description: "Bring a target app to the foreground using a shared selector.",
            input_schema: json_schema_for::<LifecycleToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: SWITCH_APP_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { switch_app(input, core, ctx).await })
        }),
    }
}

fn quit_app_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "quit-app",
            description: "Quit a target app using a shared selector.",
            input_schema: json_schema_for::<LifecycleToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: QUIT_APP_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { quit_app(input, core, ctx).await })
        }),
    }
}

fn relaunch_app_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "relaunch-app",
            description: "Quit and launch a target app using a shared selector.",
            input_schema: json_schema_for::<LifecycleToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: RELAUNCH_APP_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { relaunch_app(input, core, ctx).await })
        }),
    }
}

fn hide_app_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "hide-app",
            description: "Hide a target app using a shared selector.",
            input_schema: json_schema_for::<LifecycleToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: HIDE_APP_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { hide_app(input, core, ctx).await })
        }),
    }
}

fn unhide_app_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "unhide-app",
            description: "Unhide a target app using a shared selector.",
            input_schema: json_schema_for::<LifecycleToolInput>(),
            output_schema: json_schema_for::<ActionToolOutput>(),
            capabilities_required: UNHIDE_APP_CAPABILITIES,
            has_side_effects: true,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { unhide_app(input, core, ctx).await })
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
            build_action_request(
                Action::Click { mode: input.mode },
                input.locator,
                input.target,
                input.verification,
            ),
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
            build_action_request(
                Action::Type {
                    text: input.text,
                    clear_before: input.clear_before,
                    delay_ms: input.delay_ms,
                    trailing_keys: input.trailing_keys,
                },
                input.locator,
                input.target,
                input.verification,
            ),
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
            build_action_request(
                Action::Move,
                input.locator,
                input.target,
                input.verification,
            ),
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
            build_action_request(
                Action::Scroll {
                    delta_x: input.delta_x,
                    delta_y: input.delta_y,
                },
                input.locator,
                input.target,
                input.verification,
            ),
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
            build_action_request(
                Action::Drag {
                    from: input.from,
                    to: input.to,
                    motion: input.motion,
                },
                None,
                input.target,
                input.verification,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn swipe(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<SwipeToolInput>("swipe", input)?;
    let outcome = core
        .act(
            build_action_request(
                Action::Swipe {
                    from: input.from,
                    to: input.to,
                    duration_ms: input.duration_ms,
                    steps: input.steps,
                },
                None,
                input.target,
                input.verification,
            ),
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
            build_action_request(
                Action::Hotkey { keys: input.keys },
                None,
                input.target,
                input.verification,
            ),
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
            build_action_request(
                Action::Press {
                    key: input.key,
                    count: input.count,
                },
                None,
                input.target,
                input.verification,
            ),
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
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
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
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: input.verification.verifications,
            },
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn close_window(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<WindowTargetToolInput>("close-window", input)?;
    let outcome = core
        .act(
            build_window_geometry_action_request(
                Action::CloseWindow,
                input.target_selector,
                input.focus_policy,
                Vec::new(),
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn minimize_window(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<MinimizeWindowToolInput>("minimize-window", input)?;
    let outcome = core
        .act(
            build_window_geometry_action_request(
                Action::MinimizeWindow,
                input.target_selector,
                input.focus_policy,
                input.verification.into_verifications(),
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn maximize_window(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<WindowTargetToolInput>("maximize-window", input)?;
    let outcome = core
        .act(
            build_window_geometry_action_request(
                Action::MaximizeWindow,
                input.target_selector,
                input.focus_policy,
                Vec::new(),
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn move_window(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<MoveWindowToolInput>("move-window", input)?;
    let outcome = core
        .act(
            build_window_geometry_action_request(
                Action::MoveWindow {
                    x: input.x,
                    y: input.y,
                },
                input.target_selector,
                input.focus_policy,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn resize_window(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<ResizeWindowToolInput>("resize-window", input)?;
    let outcome = core
        .act(
            build_window_geometry_action_request(
                Action::ResizeWindow {
                    width: input.width,
                    height: input.height,
                },
                input.target_selector,
                input.focus_policy,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn set_window_bounds(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<SetWindowBoundsToolInput>("set-window-bounds", input)?;
    let outcome = core
        .act(
            build_window_geometry_action_request(
                Action::SetWindowBounds {
                    bounds: Rect {
                        x: input.x,
                        y: input.y,
                        width: input.width,
                        height: input.height,
                    },
                },
                input.target_selector,
                input.focus_policy,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn switch_app(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<LifecycleToolInput>("switch-app", input)?;
    let outcome = core
        .act(
            build_lifecycle_action_request(
                Action::SwitchApp,
                input.target_selector,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn quit_app(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<LifecycleToolInput>("quit-app", input)?;
    let outcome = core
        .act(
            build_lifecycle_action_request(
                Action::QuitApp,
                input.target_selector,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn relaunch_app(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<LifecycleToolInput>("relaunch-app", input)?;
    let outcome = core
        .act(
            build_lifecycle_action_request(
                Action::RelaunchApp,
                input.target_selector,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn hide_app(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<LifecycleToolInput>("hide-app", input)?;
    let outcome = core
        .act(
            build_lifecycle_action_request(
                Action::HideApp,
                input.target_selector,
                input.verification.verifications,
            ),
            ctx,
        )
        .await?;

    serialize_output(outcome)
}

async fn unhide_app(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<LifecycleToolInput>("unhide-app", input)?;
    let outcome = core
        .act(
            build_lifecycle_action_request(
                Action::UnhideApp,
                input.target_selector,
                input.verification.verifications,
            ),
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

fn build_action_request(
    action: Action,
    locator: Option<Locator>,
    target: ActionTargetToolInput,
    verification: ActionVerificationToolInput,
) -> ActionRequest {
    ActionRequest {
        action,
        locator,
        target_selector: target.target_selector,
        focus_policy: target.focus_policy,
        verifications: verification.verifications,
    }
}

fn build_lifecycle_action_request(
    action: Action,
    target_selector: ActionTargetSelector,
    verifications: Vec<ActionVerification>,
) -> ActionRequest {
    ActionRequest {
        action,
        locator: None,
        target_selector: Some(target_selector),
        focus_policy: ActionFocusPolicy::Auto,
        verifications,
    }
}

fn build_window_geometry_action_request(
    action: Action,
    target_selector: ActionTargetSelector,
    focus_policy: ActionFocusPolicy,
    verifications: Vec<ActionVerification>,
) -> ActionRequest {
    ActionRequest {
        action,
        locator: None,
        target_selector: Some(target_selector),
        focus_policy,
        verifications,
    }
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
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct TypeToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    text: String,
    #[serde(default)]
    clear_before: bool,
    delay_ms: Option<u64>,
    #[serde(default)]
    trailing_keys: Vec<TypeTrailingKey>,
    locator: Option<Locator>,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MoveToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    locator: Option<Locator>,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct HotkeyToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    keys: Vec<String>,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
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
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
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
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SwipeToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    from: Locator,
    to: Locator,
    duration_ms: Option<u64>,
    steps: Option<NonZeroU32>,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
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
    #[serde(flatten, default)]
    #[schemars(flatten)]
    target: ActionTargetToolInput,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
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
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct WindowTargetToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    target_selector: ActionTargetSelector,
    #[serde(default)]
    focus_policy: ActionFocusPolicy,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MinimizeWindowToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    target_selector: ActionTargetSelector,
    #[serde(default)]
    focus_policy: ActionFocusPolicy,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: WindowStateVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MoveWindowToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    target_selector: ActionTargetSelector,
    #[serde(default)]
    focus_policy: ActionFocusPolicy,
    x: f64,
    y: f64,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ResizeWindowToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    target_selector: ActionTargetSelector,
    #[serde(default)]
    focus_policy: ActionFocusPolicy,
    width: f64,
    height: f64,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SetWindowBoundsToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    target_selector: ActionTargetSelector,
    #[serde(default)]
    focus_policy: ActionFocusPolicy,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct LifecycleToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    target_selector: ActionTargetSelector,
    #[serde(flatten, default)]
    #[schemars(flatten)]
    verification: ActionVerificationToolInput,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
struct ActionTargetToolInput {
    target_selector: Option<ActionTargetSelector>,
    #[serde(default)]
    focus_policy: ActionFocusPolicy,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
struct ActionVerificationToolInput {
    #[serde(default)]
    verifications: Vec<ActionVerification>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
enum WindowStateVerification {
    WindowState,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
struct WindowStateVerificationToolInput {
    #[serde(default)]
    verifications: Vec<WindowStateVerification>,
}

impl WindowStateVerificationToolInput {
    fn into_verifications(self) -> Vec<ActionVerification> {
        self.verifications
            .into_iter()
            .map(|verification| match verification {
                WindowStateVerification::WindowState => ActionVerification::WindowState,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ActionToolOutput {
    outcome: ActionOutcome,
}

fn default_press_count() -> NonZeroU32 {
    NonZeroU32::MIN
}
