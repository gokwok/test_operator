use std::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AppInfo, Locator, Point, Rect, WindowId, WindowInfo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionRequest {
    pub action: Action,
    pub locator: Option<Locator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_selector: Option<ActionTargetSelector>,
    #[serde(default)]
    pub focus_policy: ActionFocusPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifications: Vec<ActionVerification>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum ActionTargetSelector {
    App(String),
    Pid(u32),
    WindowId(WindowId),
    WindowTitle(String),
    WindowIndex(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub enum ActionFocusPolicy {
    #[default]
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ActionVerification {
    Focus,
    WindowState,
    Geometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Action {
    Click {
        mode: ClickMode,
    },
    Move,
    Type {
        text: String,
        #[serde(default)]
        clear_before: bool,
        delay_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        trailing_keys: Vec<TypeTrailingKey>,
    },
    Press {
        key: String,
        #[serde(default = "default_press_count")]
        count: NonZeroU32,
    },
    Scroll {
        delta_x: f64,
        delta_y: f64,
    },
    Hotkey {
        keys: Vec<String>,
    },
    Drag {
        from: Locator,
        to: Locator,
        #[serde(default)]
        motion: DragMotion,
    },
    Swipe {
        from: Locator,
        to: Locator,
        duration_ms: Option<u64>,
        steps: Option<NonZeroU32>,
    },
    LaunchApp {
        bundle_id_or_name: String,
    },
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    MoveWindow {
        x: f64,
        y: f64,
    },
    ResizeWindow {
        width: f64,
        height: f64,
    },
    SetWindowBounds {
        bounds: crate::Rect,
    },
    SwitchApp,
    QuitApp,
    RelaunchApp,
    HideApp,
    UnhideApp,
    FocusWindow {
        id: WindowId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct DragMotion {
    pub duration_ms: Option<u64>,
    pub steps: Option<NonZeroU32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<DragModifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DragModifier {
    Command,
    Control,
    Option,
    Shift,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TypeTrailingKey {
    Return,
    Tab,
    Escape,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ClickMode {
    Left,
    Right,
    Middle,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

fn default_press_count() -> NonZeroU32 {
    NonZeroU32::MIN
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionOutcome {
    pub success: bool,
    pub duration_ms: u64,
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<ActionCoordinates>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_app: Option<AppInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_window: Option<WindowInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<ActionSideEffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionCoordinates {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Point>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "data")]
pub enum ActionSideEffect {
    Click {
        mode: ClickMode,
    },
    MoveCursor,
    Type {
        clear_before: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        trailing_keys: Vec<TypeTrailingKey>,
    },
    Press {
        key: String,
        count: u32,
    },
    Scroll {
        delta_x: f64,
        delta_y: f64,
    },
    Hotkey {
        keys: Vec<String>,
    },
    Drag {
        motion: DragMotion,
    },
    Swipe {
        duration_ms: Option<u64>,
        steps: Option<NonZeroU32>,
    },
    LaunchApp,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    MoveWindow {
        bounds: Rect,
    },
    ResizeWindow {
        bounds: Rect,
    },
    SetWindowBounds {
        bounds: Rect,
    },
    SwitchApp,
    QuitApp,
    RelaunchApp,
    HideApp,
    UnhideApp,
    FocusWindow,
}
