use crate::{Capability, ElementId, SnapshotId};

#[derive(Debug, thiserror::Error)]
pub enum OperatorError {
    #[error("capability not supported: {0:?}")]
    CapabilityNotSupported(Capability),

    #[error("target not found: {0}")]
    TargetNotFound(String),

    #[error("target is busy (queue timeout)")]
    TargetBusy,

    #[error("operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("element not found: {0:?}")]
    ElementNotFound(ElementId),

    #[error("snapshot not found: {0:?}")]
    SnapshotNotFound(SnapshotId),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("platform error: {0}")]
    Platform(String),

    #[error("tool error: {tool}, message: {message}")]
    Tool { tool: String, message: String },

    #[error("model error: {0}")]
    Model(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
