use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use operator_core::DriverConfig;
use serde_json::Value;

use crate::HarmonyConfigError;

const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_STARTUP_DELAY_MS: u64 = 500;
const DEFAULT_REMOTE_AGENT_PATH: &str = "/data/local/tmp/agent.so";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarmonyHdcConfig {
    addr: String,
    connect_key: String,
    key_dir: Option<PathBuf>,
    timeout: Duration,
    agent_path: Option<PathBuf>,
    remote_agent_path: String,
    startup_delay: Duration,
}

impl HarmonyHdcConfig {
    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn connect_key(&self) -> &str {
        &self.connect_key
    }

    pub fn key_dir(&self) -> Option<&Path> {
        self.key_dir.as_deref()
    }

    pub fn agent_path(&self) -> Option<&Path> {
        self.agent_path.as_deref()
    }

    pub fn remote_agent_path(&self) -> &str {
        &self.remote_agent_path
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn startup_delay(&self) -> Duration {
        self.startup_delay
    }
}

impl TryFrom<&DriverConfig> for HarmonyHdcConfig {
    type Error = HarmonyConfigError;

    fn try_from(config: &DriverConfig) -> Result<Self, Self::Error> {
        for key in config.keys() {
            match key.as_str() {
                "addr" | "connect_key" | "key_dir" | "timeout_ms" | "agent_path"
                | "remote_agent_path" | "startup_delay_ms" => {}
                other => return Err(HarmonyConfigError::unknown(other)),
            }
        }

        let addr = required_string(config, "addr")?;
        Ok(Self {
            connect_key: optional_string(config, "connect_key")?.unwrap_or_else(|| addr.clone()),
            addr,
            key_dir: optional_path(config, "key_dir")?,
            timeout: Duration::from_millis(
                optional_u64(config, "timeout_ms")?.unwrap_or(DEFAULT_TIMEOUT_MS),
            ),
            agent_path: optional_path(config, "agent_path")?,
            remote_agent_path: optional_string(config, "remote_agent_path")?
                .unwrap_or_else(|| DEFAULT_REMOTE_AGENT_PATH.to_string()),
            startup_delay: Duration::from_millis(
                optional_u64(config, "startup_delay_ms")?.unwrap_or(DEFAULT_STARTUP_DELAY_MS),
            ),
        })
    }
}

fn required_string(
    config: &DriverConfig,
    field: &'static str,
) -> Result<String, HarmonyConfigError> {
    optional_string(config, field)?.ok_or_else(|| HarmonyConfigError::missing(field))
}

fn optional_string(
    config: &DriverConfig,
    field: &'static str,
) -> Result<Option<String>, HarmonyConfigError> {
    match config.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => {
            if value.trim().is_empty() {
                return Err(HarmonyConfigError::invalid(field, "non-empty string"));
            }
            Ok(Some(value.clone()))
        }
        Some(_) => Err(HarmonyConfigError::invalid(field, "non-empty string")),
    }
}

fn optional_path(
    config: &DriverConfig,
    field: &'static str,
) -> Result<Option<PathBuf>, HarmonyConfigError> {
    optional_string(config, field).map(|value| value.map(PathBuf::from))
}

fn optional_u64(
    config: &DriverConfig,
    field: &'static str,
) -> Result<Option<u64>, HarmonyConfigError> {
    match config.get(field) {
        None => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| HarmonyConfigError::invalid(field, "unsigned integer")),
        Some(_) => Err(HarmonyConfigError::invalid(field, "unsigned integer")),
    }
}

#[cfg(test)]
mod tests {
    use operator_core::DriverConfig;
    use serde_json::json;

    use super::HarmonyHdcConfig;
    use crate::HarmonyConfigError;

    #[test]
    fn parses_supported_harmony_hdc_driver_config() {
        let config = DriverConfig::from([
            ("addr".into(), json!("192.168.8.43:35319")),
            ("connect_key".into(), json!("pc-01")),
            ("key_dir".into(), json!("/Users/gokwok/.hdc")),
            ("timeout_ms".into(), json!(45_000_u64)),
            ("agent_path".into(), json!("/tmp/agent.so")),
            (
                "remote_agent_path".into(),
                json!("/data/local/tmp/agent.so"),
            ),
            ("startup_delay_ms".into(), json!(800_u64)),
        ]);

        let parsed = HarmonyHdcConfig::try_from(&config).expect("config should parse");

        assert_eq!(parsed.addr(), "192.168.8.43:35319");
        assert_eq!(parsed.connect_key(), "pc-01");
        assert_eq!(
            parsed
                .key_dir()
                .map(|path| path.to_string_lossy().into_owned()),
            Some("/Users/gokwok/.hdc".into())
        );
        assert_eq!(
            parsed
                .agent_path()
                .map(|path| path.to_string_lossy().into_owned()),
            Some("/tmp/agent.so".into())
        );
        assert_eq!(parsed.remote_agent_path(), "/data/local/tmp/agent.so");
        assert_eq!(parsed.timeout().as_millis(), 45_000);
        assert_eq!(parsed.startup_delay().as_millis(), 800);
    }

    #[test]
    fn rejects_unknown_harmony_hdc_driver_config_fields() {
        let config = DriverConfig::from([
            ("addr".into(), json!("192.168.8.43:35319")),
            ("endpoint".into(), json!("ws://127.0.0.1:9000")),
        ]);

        let error = HarmonyHdcConfig::try_from(&config).expect_err("unknown field should fail");

        assert_eq!(
            error,
            HarmonyConfigError::UnknownField {
                field: "endpoint".into()
            }
        );
    }

    #[test]
    fn normalizes_defaulted_harmony_hdc_driver_config_fields() {
        let config = DriverConfig::from([("addr".into(), json!("192.168.8.43:35319"))]);

        let parsed = HarmonyHdcConfig::try_from(&config).expect("config should parse");

        assert_eq!(parsed.connect_key(), "192.168.8.43:35319");
        assert_eq!(parsed.agent_path(), None);
        assert_eq!(parsed.remote_agent_path(), "/data/local/tmp/agent.so");
        assert_eq!(parsed.timeout().as_millis(), 60_000);
        assert_eq!(parsed.startup_delay().as_millis(), 500);
    }
}
