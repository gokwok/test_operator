use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::TargetId;

pub type DriverConfig = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetDescriptor {
    pub id: TargetId,
    pub platform: String,
    pub driver: String,
    #[serde(default)]
    pub driver_config: DriverConfig,
}
