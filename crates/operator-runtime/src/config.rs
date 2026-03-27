use std::collections::BTreeMap;

use operator_core::{DriverConfig, TargetId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedTargetConfig {
    pub platform: String,
    pub driver: String,
    #[serde(default)]
    pub driver_config: DriverConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub default_target: TargetId,
    #[serde(default = "default_named_targets")]
    pub targets: BTreeMap<String, NamedTargetConfig>,
    pub snapshot_ttl_hours: u64,
    pub max_snapshots: usize,
    pub default_timeout_ms: u64,
    pub audit_enabled: bool,
    pub allow_side_effects: bool,
    pub redact_sensitive_fields: bool,
    pub artifact_ttl_hours: u64,
    pub snapshot_evict_interval: u32,
}

fn default_named_targets() -> BTreeMap<String, NamedTargetConfig> {
    BTreeMap::from([(
        "macos".into(),
        NamedTargetConfig {
            platform: "macos".into(),
            driver: "macos.system".into(),
            driver_config: DriverConfig::new(),
        },
    )])
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_target: TargetId("macos".into()),
            targets: default_named_targets(),
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
