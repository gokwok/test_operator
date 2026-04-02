use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ElementId, Point, SnapshotId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Locator {
    SnapshotElement {
        snapshot: SnapshotId,
        element: ElementId,
    },
    SnapshotPixelCoords {
        snapshot: SnapshotId,
        point: Point,
    },
    SnapshotCoords {
        snapshot: SnapshotId,
        point: Point,
    },
    SnapshotNormalizedCoords {
        snapshot: SnapshotId,
        point: Point,
        basis: f64,
    },
    /// Search a snapshot's element tree by label/value text and click the
    /// centre of the first match.  Use `snapshot: SnapshotId("latest")` to
    /// automatically pick the most recent snapshot for the target.
    SnapshotText {
        snapshot: SnapshotId,
        text: String,
    },
    /// Raw platform-driver text search (live inspection, no snapshot).
    /// Prefer [`Locator::SnapshotText`] when a snapshot is available.
    Text(String),
    Role {
        role: String,
        index: usize,
    },
    Coords(Point),
}
