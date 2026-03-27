use serde::{Deserialize, Serialize};

use crate::TargetId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetDescriptor {
    pub id: TargetId,
    pub platform: String,
    pub driver: String,
}
