use serde::{Deserialize, Serialize};

use crate::{Rect, WindowId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    pub kind: SurfaceKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceKind {
    Fullscreen { display_id: Option<u32> },
    Frontmost,
    Window { id: WindowId },
    Region { rect: Rect },
}
