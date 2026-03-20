use operator_core::{OperatorError, TargetConnection, TargetDescriptor, TargetId};

#[derive(Debug, Clone)]
pub struct TargetResolver {
    default_target: TargetId,
}

impl TargetResolver {
    pub fn new(default_target: TargetId) -> Self {
        Self { default_target }
    }

    pub fn resolve(&self, target: Option<&TargetId>) -> Result<TargetDescriptor, OperatorError> {
        let target = target.unwrap_or(&self.default_target);
        parse_target(target)
    }
}

fn parse_target(target: &TargetId) -> Result<TargetDescriptor, OperatorError> {
    let parts = target.0.split(':').collect::<Vec<_>>();

    match parts.as_slice() {
        ["local", platform] if !platform.is_empty() => Ok(TargetDescriptor {
            id: target.clone(),
            platform: (*platform).to_string(),
            device_id: None,
            connection: TargetConnection::Local,
        }),
        ["device", platform, device_id] if !platform.is_empty() && !device_id.is_empty() => {
            Ok(TargetDescriptor {
                id: target.clone(),
                platform: (*platform).to_string(),
                device_id: Some((*device_id).to_string()),
                connection: TargetConnection::Bridge { endpoint: None },
            })
        }
        _ => Err(OperatorError::TargetNotFound(target.to_string())),
    }
}
