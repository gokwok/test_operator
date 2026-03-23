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
    Click { mode: ClickMode },
    Type { text: String },
    Scroll { delta_x: f64, delta_y: f64 },
    Hotkey { keys: Vec<String> },
    Drag { from: Locator, to: Locator },
    LaunchApp { bundle_id_or_name: String },
    FocusWindow { id: WindowId },
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
