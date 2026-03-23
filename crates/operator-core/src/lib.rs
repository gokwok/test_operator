//! Core domain primitives for Operator.

mod action;
mod capability;
mod driver;
mod error;
mod exec;
mod geometry;
mod ids;
mod locator;
mod query;
mod snapshot;
mod surface;
mod target;

pub use action::{
    Action, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionTargetSelector, ClickMode,
    DragModifier, DragMotion, MouseButton, TypeTrailingKey,
};
pub use capability::{Capability, CapabilityId, CapabilitySet};
pub use driver::{HealthStatus, PlatformDriver};
pub use error::OperatorError;
pub use exec::{ExecContext, ObserveRequest, ObserveResult};
pub use geometry::{Point, Rect};
pub use ids::{ArtifactId, ElementId, SessionId, SnapshotId, TargetId, WindowId};
pub use locator::Locator;
pub use query::{
    AppInfo, FocusInfo, PermissionStatus, PermissionsReport, QueryRequest, QueryResult, WindowInfo,
};
pub use snapshot::{ElementSource, Snapshot, SnapshotMetadata, UiElement};
pub use surface::{Surface, SurfaceKind};
pub use target::{TargetConnection, TargetDescriptor};
