//! Core domain primitives for Operator.

mod error;
mod geometry;
mod ids;
mod target;

pub use error::OperatorError;
pub use geometry::{Point, Rect};
pub use ids::{ArtifactId, ElementId, SessionId, SnapshotId, TargetId, WindowId};
pub use target::{TargetConnection, TargetDescriptor};
