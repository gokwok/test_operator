use std::collections::BTreeMap;

use operator_core::{DriverConfig, OperatorError, TargetDescriptor, TargetId};
use serde_json::Value;

use crate::NamedTargetConfig;

#[derive(Debug, Clone)]
pub struct TargetResolver {
    default_target: TargetId,
    named_targets: BTreeMap<String, NamedTargetConfig>,
}

impl TargetResolver {
    pub fn new(
        default_target: TargetId,
        named_targets: BTreeMap<String, NamedTargetConfig>,
    ) -> Self {
        Self {
            default_target,
            named_targets,
        }
    }

    pub fn resolve(&self, target: Option<&TargetId>) -> Result<TargetDescriptor, OperatorError> {
        let target = target.unwrap_or(&self.default_target);
        self.resolve_named(target)
            .or_else(|| parse_legacy_target(target))
            .ok_or_else(|| OperatorError::TargetNotFound(target.to_string()))
    }

    fn resolve_named(&self, target: &TargetId) -> Option<TargetDescriptor> {
        self.named_targets
            .get(target.0.as_str())
            .map(|descriptor| TargetDescriptor {
                id: target.clone(),
                platform: descriptor.platform.clone(),
                driver: descriptor.driver.clone(),
                driver_config: descriptor.driver_config.clone(),
            })
    }
}

fn parse_legacy_target(target: &TargetId) -> Option<TargetDescriptor> {
    let parts = target.0.split(':').collect::<Vec<_>>();

    match parts.as_slice() {
        ["local", platform] if !platform.is_empty() => Some(TargetDescriptor {
            id: target.clone(),
            platform: (*platform).to_string(),
            driver: format!("{platform}.system"),
            driver_config: DriverConfig::new(),
        }),
        ["device", platform, device_id] if !platform.is_empty() && !device_id.is_empty() => {
            Some(TargetDescriptor {
                id: target.clone(),
                platform: (*platform).to_string(),
                driver: format!("{platform}.bridge"),
                driver_config: DriverConfig::from([(
                    "device_id".into(),
                    Value::String((*device_id).to_string()),
                )]),
            })
        }
        _ => None,
    }
}
