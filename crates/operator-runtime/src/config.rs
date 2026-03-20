use operator_core::TargetId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub default_target: TargetId,
    pub snapshot_ttl_hours: u64,
    pub max_snapshots: usize,
    pub default_timeout_ms: u64,
    pub audit_enabled: bool,
    pub allow_side_effects: bool,
    pub redact_sensitive_fields: bool,
    pub artifact_ttl_hours: u64,
    pub snapshot_evict_interval: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_target: TargetId("local:macos".into()),
            snapshot_ttl_hours: 24,
            max_snapshots: 200,
            default_timeout_ms: 10_000,
            audit_enabled: true,
            allow_side_effects: true,
            redact_sensitive_fields: true,
            artifact_ttl_hours: 24,
            snapshot_evict_interval: 100,
        }
    }
}
