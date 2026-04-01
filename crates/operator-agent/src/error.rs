use operator_core::OperatorError;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agent configuration error: {0}")]
    Config(String),

    #[error("agent run interrupted: {0}")]
    Interrupted(String),

    #[error("model is not configured: {0}")]
    ModelNotConfigured(String),

    #[error("planner error: {0}")]
    Planner(String),

    #[error("runtime error: {0}")]
    Runtime(#[from] OperatorError),
}
