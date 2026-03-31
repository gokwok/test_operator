mod config_document;

use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

pub use config_document::{
    parse_model_set_expression, parse_target_set_expression, ModelConfigFieldPath,
    RuntimeConfigDocument, TargetConfigFieldPath,
};
use operator_core::{OperatorError, PlatformDriver, TargetDescriptor, TargetId};
use operator_platform_harmony::HarmonyHdcDriverFactory;
use operator_platform_macos::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};
use operator_runtime::{NamedTargetConfig, PlatformDriverFactory, PlatformRegistry, RuntimeConfig};
use serde::Deserialize;

const OPENAI_MODEL_SELECTOR: &str = "openai";
const DOUBAO_MODEL_SELECTOR: &str = "doubao";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BootstrapConfig {
    pub runtime: RuntimeConfig,
    pub agent_model: AgentModelConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentModelConfig {
    pub default: Option<String>,
    pub providers: BTreeMap<String, AgentModelProviderConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentModelProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
}

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

pub fn load_bootstrap_config() -> Result<BootstrapConfig, OperatorError> {
    load_bootstrap_config_from(operator_home_dir())
}

pub fn load_bootstrap_config_from(
    operator_home: impl AsRef<Path>,
) -> Result<BootstrapConfig, OperatorError> {
    let path = runtime_config_path(operator_home);
    RuntimeConfigDocument::load(path)?.to_bootstrap_config()
}

pub fn load_runtime_config() -> Result<RuntimeConfig, OperatorError> {
    load_runtime_config_from(operator_home_dir())
}

pub fn load_runtime_config_from(
    operator_home: impl AsRef<Path>,
) -> Result<RuntimeConfig, OperatorError> {
    Ok(load_bootstrap_config_from(operator_home)?.runtime)
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

pub(crate) fn parse_runtime_config(
    contents: &str,
    path: &Path,
) -> Result<RuntimeConfig, OperatorError> {
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

pub(crate) fn parse_bootstrap_config(
    contents: &str,
    path: &Path,
) -> Result<BootstrapConfig, OperatorError> {
    let runtime = parse_runtime_config(contents, path)?;
    let root = toml::from_str::<toml::Table>(contents).map_err(|error| {
        OperatorError::Platform(format!(
            "invalid runtime config at {}: {error}",
            path.display()
        ))
    })?;
    let agent_model = parse_agent_model_config(&root)?;

    Ok(BootstrapConfig {
        runtime,
        agent_model,
    })
}

fn parse_agent_model_config(root: &toml::Table) -> Result<AgentModelConfig, OperatorError> {
    let Some(agent) = root.get("agent") else {
        return Ok(AgentModelConfig::default());
    };
    let Some(agent_table) = agent.as_table() else {
        return Ok(AgentModelConfig::default());
    };
    let Some(model_item) = agent_table.get("model") else {
        return Ok(AgentModelConfig::default());
    };
    let Some(model_table) = model_item.as_table() else {
        // Preserve legacy `[agent] model = "gpt-5.4"` style configs until the agent bootstrap
        // path migrates to the new config-backed selector contract.
        return Ok(AgentModelConfig::default());
    };

    let mut config = AgentModelConfig::default();
    for key in model_table.keys() {
        if key != "default" && key != "provider" {
            return Err(OperatorError::Platform(format!(
                "unknown agent model field `{key}`; expected `default` or `provider`"
            )));
        }
    }

    if let Some(default_item) = model_table.get("default") {
        let selector = default_item.as_str().ok_or_else(|| {
            OperatorError::Platform("agent model default selector must be a string".into())
        })?;
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(OperatorError::Platform(
                "agent model default selector must not be empty".into(),
            ));
        }
        validate_supported_model_selector(selector)?;
        config.default = Some(selector.to_owned());
    }

    if let Some(provider_item) = model_table.get("provider") {
        let provider_table = provider_item.as_table().ok_or_else(|| {
            OperatorError::Platform("agent model provider section must be a table".into())
        })?;
        for (name, item) in provider_table {
            validate_supported_model_selector(name)?;
            let item_table = item.as_table().ok_or_else(|| {
                OperatorError::Platform(format!("agent model provider `{name}` must be a table"))
            })?;
            config
                .providers
                .insert(name.clone(), parse_agent_model_provider(name, item_table)?);
        }
    }

    Ok(config)
}

fn parse_agent_model_provider(
    name: &str,
    table: &toml::Table,
) -> Result<AgentModelProviderConfig, OperatorError> {
    let mut provider = AgentModelProviderConfig::default();
    for (field, value) in table {
        let slot = match field.as_str() {
            "api_key" => &mut provider.api_key,
            "base_url" => &mut provider.base_url,
            "model_name" => &mut provider.model_name,
            _ => {
                return Err(OperatorError::Platform(format!(
                    "unknown agent model provider field `{field}` for `{name}`"
                )))
            }
        };
        let string = value.as_str().ok_or_else(|| {
            OperatorError::Platform(format!(
                "agent model provider field `{name}.{field}` must be a string"
            ))
        })?;
        *slot = normalized_optional_string(string);
    }

    Ok(provider)
}

pub(crate) fn validate_supported_model_selector(selector: &str) -> Result<(), OperatorError> {
    if matches!(selector, OPENAI_MODEL_SELECTOR | DOUBAO_MODEL_SELECTOR) {
        Ok(())
    } else {
        Err(OperatorError::Platform(format!(
            "unsupported agent model selector `{selector}`; expected one of: {OPENAI_MODEL_SELECTOR}, {DOUBAO_MODEL_SELECTOR}"
        )))
    }
}

fn normalized_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
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
    use super::{
        load_bootstrap_config_from, load_runtime_config_from, runtime_config_path,
        AgentModelProviderConfig,
    };
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
description = "Shared Windows lab"

[targets.windows-lab.driver_config]
endpoint = "wss://windows-lab.internal"

[targets.harmony-phone]
platform = "harmony"
driver = "harmony.node"
description = "Harmony phone"

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
            windows_target.description.as_deref(),
            Some("Shared Windows lab")
        );
        assert_eq!(
            windows_target.driver_config,
            DriverConfig::from([("endpoint".into(), json!("wss://windows-lab.internal"))])
        );
        assert_eq!(harmony_target.platform, "harmony");
        assert_eq!(harmony_target.driver, "harmony.node");
        assert_eq!(harmony_target.description.as_deref(), Some("Harmony phone"));
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
    fn load_bootstrap_config_parses_agent_model_section() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        std::fs::write(
            &path,
            r#"
[agent]
max_steps = 50

[agent.model]
default = "openai"

[agent.model.provider.openai]
api_key = " sk-openai "
base_url = " https://api.openai.com/v1 "
model_name = " gpt-5.4 "

[agent.model.provider.doubao]
api_key = ""
base_url = "https://ark.cn-beijing.volces.com/api/v3"
model_name = "doubao-seed-2-0-lite-260215"
"#,
        )
        .expect("write config");

        let config = load_bootstrap_config_from(temp.path()).expect("load bootstrap config");

        assert_eq!(config.agent_model.default.as_deref(), Some("openai"));
        assert_eq!(
            config.agent_model.providers.get("openai"),
            Some(&AgentModelProviderConfig {
                api_key: Some("sk-openai".into()),
                base_url: Some("https://api.openai.com/v1".into()),
                model_name: Some("gpt-5.4".into()),
            })
        );
        assert_eq!(
            config.agent_model.providers.get("doubao"),
            Some(&AgentModelProviderConfig {
                api_key: None,
                base_url: Some("https://ark.cn-beijing.volces.com/api/v3".into()),
                model_name: Some("doubao-seed-2-0-lite-260215".into()),
            })
        );
    }

    #[test]
    fn load_bootstrap_config_rejects_unsupported_agent_model_provider_names() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        std::fs::write(
            &path,
            r#"
[agent.model]
default = "openai"

[agent.model.provider.anthropic]
api_key = "sk-ant-123"
"#,
        )
        .expect("write config");

        let error = load_bootstrap_config_from(temp.path()).expect_err("provider name should fail");
        assert!(error
            .to_string()
            .contains("unsupported agent model selector `anthropic`"));
    }

    #[test]
    fn load_bootstrap_config_rejects_unknown_agent_model_provider_fields() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        std::fs::write(
            &path,
            r#"
[agent.model]
default = "openai"

[agent.model.provider.openai]
api_key = "sk-openai"
region = "us-east-1"
"#,
        )
        .expect("write config");

        let error =
            load_bootstrap_config_from(temp.path()).expect_err("provider field should fail");
        assert!(error
            .to_string()
            .contains("unknown agent model provider field `region` for `openai`"));
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
