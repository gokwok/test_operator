use serde::{Deserialize, Serialize};

use crate::{ElementId, Point, SnapshotId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Locator {
    SnapshotElement {
        snapshot: SnapshotId,
        element: ElementId,
    },
    Text(String),
    Role {
        role: String,
        index: usize,
    },
    Coords(Point),
}
