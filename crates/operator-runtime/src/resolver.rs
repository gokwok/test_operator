use std::collections::BTreeMap;

use operator_core::{OperatorError, TargetDescriptor, TargetId};

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
