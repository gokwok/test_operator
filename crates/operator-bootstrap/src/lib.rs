use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use operator_core::{OperatorError, PlatformDriver, TargetDescriptor, TargetId};
use operator_platform_harmony::HarmonyHdcDriverFactory;
use operator_platform_macos::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};
use operator_runtime::{NamedTargetConfig, PlatformDriverFactory, PlatformRegistry, RuntimeConfig};
use serde::Deserialize;

pub fn operator_home_dir() -> PathBuf {
    if let Some(path) = env::var_os("OPERATOR_HOME") {
        return PathBuf::from(path);
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".operator");
    }

    PathBuf::from(".operator")
}

pub fn runtime_config_path(operator_home: impl AsRef<Path>) -> PathBuf {
    operator_home.as_ref().join("config.toml")
}

pub fn load_runtime_config() -> Result<RuntimeConfig, OperatorError> {
    load_runtime_config_from(operator_home_dir())
}

pub fn load_runtime_config_from(
    operator_home: impl AsRef<Path>,
) -> Result<RuntimeConfig, OperatorError> {
    let path = runtime_config_path(operator_home);
    if !path.exists() {
        return Ok(RuntimeConfig::default());
    }

    let contents = fs::read_to_string(&path)?;
    parse_runtime_config(&contents, &path)
}

pub fn system_platform_registry(artifacts_dir: impl AsRef<Path>) -> PlatformRegistry {
    let mut registry = PlatformRegistry::new();
    registry.register_factory(Arc::new(HarmonyHdcDriverFactory::new_with_artifacts_dir(
        artifacts_dir.as_ref(),
    )));
    registry.register_factory(Arc::new(MacosSystemDriverFactory::new(artifacts_dir)));
    registry
}

struct MacosSystemDriverFactory {
    artifacts_dir: PathBuf,
}

impl MacosSystemDriverFactory {
    fn new(artifacts_dir: impl AsRef<Path>) -> Self {
        Self {
            artifacts_dir: artifacts_dir.as_ref().to_path_buf(),
        }
    }
}

impl PlatformDriverFactory for MacosSystemDriverFactory {
    fn driver_id(&self) -> &str {
        "macos.system"
    }

    fn build(&self, target: &TargetDescriptor) -> Result<Arc<dyn PlatformDriver>, OperatorError> {
        if target.platform != "macos" {
            return Err(OperatorError::Platform(format!(
                "target {} resolved to platform {}, but macos.system only supports macos",
                target.id, target.platform
            )));
        }

        if !target.driver_config.is_empty() {
            return Err(OperatorError::Platform(format!(
                "driver macos.system does not accept target-level driver_config: {}",
                serde_json::to_string(&target.driver_config)?
            )));
        }

        Ok(Arc::new(MacosDriver::with_observe(
            SystemAppService,
            SystemPermissionReader,
            SystemCaptureProvider::new(&self.artifacts_dir),
            SystemTreeInspector,
        )) as Arc<dyn PlatformDriver>)
    }
}

fn parse_runtime_config(contents: &str, path: &Path) -> Result<RuntimeConfig, OperatorError> {
    let file = toml::from_str::<RuntimeConfigFile>(contents).map_err(|error| {
        OperatorError::Platform(format!(
            "invalid runtime config at {}: {error}",
            path.display()
        ))
    })?;

    let mut config = RuntimeConfig::default();

    if let Some(default_target) = file.runtime.default_target {
        config.default_target = default_target;
    }
    if let Some(snapshot_ttl_hours) = file.runtime.snapshot_ttl_hours {
        config.snapshot_ttl_hours = snapshot_ttl_hours;
    }
    if let Some(max_snapshots) = file.runtime.max_snapshots {
        config.max_snapshots = max_snapshots;
    }
    if let Some(default_timeout_ms) = file.runtime.default_timeout_ms {
        config.default_timeout_ms = default_timeout_ms;
    }
    if let Some(snapshot_evict_interval) = file.runtime.snapshot_evict_interval {
        config.snapshot_evict_interval = snapshot_evict_interval;
    }

    if let Some(audit_enabled) = file.security.audit_enabled {
        config.audit_enabled = audit_enabled;
    }
    if let Some(allow_side_effects) = file.security.allow_side_effects {
        config.allow_side_effects = allow_side_effects;
    }
    if let Some(redact_sensitive_fields) = file.security.redact_sensitive_fields {
        config.redact_sensitive_fields = redact_sensitive_fields;
    }
    if let Some(artifact_ttl_hours) = file.security.artifact_ttl_hours {
        config.artifact_ttl_hours = artifact_ttl_hours;
    }

    config.targets.extend(file.targets);
    Ok(config)
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeConfigFile {
    #[serde(default)]
    runtime: RuntimeSection,
    #[serde(default)]
    security: SecuritySection,
    #[serde(default)]
    targets: std::collections::BTreeMap<String, NamedTargetConfig>,
    #[serde(flatten)]
    _extra_sections: std::collections::BTreeMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RuntimeSection {
    default_target: Option<TargetId>,
    snapshot_ttl_hours: Option<u64>,
    max_snapshots: Option<usize>,
    default_timeout_ms: Option<u64>,
    snapshot_evict_interval: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SecuritySection {
    audit_enabled: Option<bool>,
    allow_side_effects: Option<bool>,
    redact_sensitive_fields: Option<bool>,
    artifact_ttl_hours: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::{load_runtime_config_from, runtime_config_path};
    use operator_core::{DriverConfig, TargetDescriptor, TargetId};
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn load_runtime_config_uses_defaults_when_config_file_is_missing() {
        let temp = tempdir().expect("tempdir");
        let config = load_runtime_config_from(temp.path()).expect("load config");

        assert_eq!(config, operator_runtime::RuntimeConfig::default());
    }

    #[test]
    fn load_runtime_config_merges_runtime_security_and_named_targets() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        std::fs::write(
            &path,
            r#"
[runtime]
default_target = "windows-lab"
default_timeout_ms = 250
max_snapshots = 32
snapshot_ttl_hours = 12

[security]
allow_side_effects = false
audit_enabled = false
redact_sensitive_fields = false
artifact_ttl_hours = 48

[targets.windows-lab]
platform = "windows"
driver = "windows.remote"

[targets.windows-lab.driver_config]
endpoint = "wss://windows-lab.internal"

[targets.harmony-phone]
platform = "harmony"
driver = "harmony.node"

[targets.harmony-phone.driver_config]
node = "phone-01"

[agent]
model = "gpt-5.4"
"#,
        )
        .expect("write config");

        let config = load_runtime_config_from(temp.path()).expect("load config");
        let windows_target = config.targets.get("windows-lab").expect("windows target");
        let harmony_target = config.targets.get("harmony-phone").expect("harmony target");

        assert_eq!(config.default_target, TargetId("windows-lab".into()));
        assert_eq!(config.default_timeout_ms, 250);
        assert_eq!(config.max_snapshots, 32);
        assert_eq!(config.snapshot_ttl_hours, 12);
        assert!(!config.allow_side_effects);
        assert!(!config.audit_enabled);
        assert!(!config.redact_sensitive_fields);
        assert_eq!(config.artifact_ttl_hours, 48);
        assert!(
            config.targets.contains_key("macos"),
            "built-in macos target should remain available"
        );
        assert_eq!(windows_target.platform, "windows");
        assert_eq!(windows_target.driver, "windows.remote");
        assert_eq!(
            windows_target.driver_config,
            DriverConfig::from([("endpoint".into(), json!("wss://windows-lab.internal"))])
        );
        assert_eq!(harmony_target.platform, "harmony");
        assert_eq!(harmony_target.driver, "harmony.node");
        assert_eq!(
            harmony_target.driver_config,
            DriverConfig::from([("node".into(), json!("phone-01"))])
        );
    }

    #[test]
    fn load_runtime_config_rejects_invalid_named_target_fields() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        std::fs::write(
            &path,
            r#"
[targets.windows-lab]
platform = "windows"
driver = "windows.remote"
endpoint = "wss://windows-lab.internal"
"#,
        )
        .expect("write config");

        let error = load_runtime_config_from(temp.path()).expect_err("config should fail");
        let rendered = error.to_string();
        assert!(rendered.contains("invalid runtime config"));
        assert!(rendered.contains("unknown field `endpoint`"));
    }

    #[test]
    fn system_platform_registry_registers_harmony_hdc_factory() {
        let registry = super::system_platform_registry("/tmp/operator-artifacts");
        let factory = registry.factory("harmony.hdc").expect("harmony factory");
        let driver = factory
            .build(&TargetDescriptor {
                id: TargetId("harmony-pc".into()),
                platform: "harmony".into(),
                driver: "harmony.hdc".into(),
                driver_config: DriverConfig::from([("addr".into(), json!("192.168.8.43:35319"))]),
            })
            .expect("factory should build harmony scaffold");

        assert_eq!(driver.platform_id(), "harmony");
        assert_eq!(driver.driver_id(), "harmony.hdc");
    }
}
