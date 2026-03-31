use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlannerFormat {
    #[default]
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub default_model: String,
    pub max_steps: u32,
    pub max_parse_attempts: u32,
    pub repeated_error_limit: u32,
    pub step_timeout_ms: u64,
    pub planner_format: PlannerFormat,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_model: "openai".into(),
            max_steps: 40,
            max_parse_attempts: 3,
            repeated_error_limit: 3,
            step_timeout_ms: 30_000,
            planner_format: PlannerFormat::Json,
        }
    }
}
