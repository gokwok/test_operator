use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Rect, WindowId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Surface {
    pub kind: SurfaceKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum SurfaceKind {
    Fullscreen { display_id: Option<u32> },
    Frontmost,
    Window { id: WindowId },
    Region { rect: Rect },
}
