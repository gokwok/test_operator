use operator_runtime::ToolSpec as RuntimeToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}

impl From<RuntimeToolSpec> for AgentToolSpec {
    fn from(spec: RuntimeToolSpec) -> Self {
        Self {
            name: spec.name.to_string(),
            description: spec.description.to_string(),
            input_schema: spec.input_schema,
            read_only: !spec.has_side_effects,
        }
    }
}
