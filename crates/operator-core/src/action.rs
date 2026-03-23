use std::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Locator, WindowId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionRequest {
    pub action: Action,
    pub locator: Option<Locator>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Action {
    Click {
        mode: ClickMode,
    },
    Type {
        text: String,
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
    LaunchApp {
        bundle_id_or_name: String,
    },
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActionOutcome {
    pub success: bool,
    pub duration_ms: u64,
    pub detail: Option<String>,
}
