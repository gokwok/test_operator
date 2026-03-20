use serde::{Deserialize, Serialize};

use crate::TargetId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetDescriptor {
    pub id: TargetId,
    pub platform: String,
    pub device_id: Option<String>,
    pub connection: TargetConnection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetConnection {
    Local,
    Bridge { endpoint: Option<String> },
}
