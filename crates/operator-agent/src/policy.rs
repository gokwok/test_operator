use serde_json::json;

use crate::{
    session::{AgentMessage, AgentSessionState},
    tools::AgentToolResult,
    AgentError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlannerFailureStage {
    Parse,
    Validation,
}

impl PlannerFailureStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Validation => "validation",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannerRetryDecision {
    Retry {
        retry_count: u32,
        retries_remaining: u32,
    },
    Stop {
        retry_count: u32,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannerRetryPolicy {
    max_parse_attempts: u32,
}

impl PlannerRetryPolicy {
    pub fn new(max_parse_attempts: u32) -> Self {
        Self { max_parse_attempts }
    }

    pub fn register_failure(
        &self,
        state: &mut AgentSessionState,
        stage: PlannerFailureStage,
        error: &AgentError,
    ) -> PlannerRetryDecision {
        let retry_count = state.bump_parse_attempts();
        let retries_remaining = self.max_parse_attempts.saturating_sub(retry_count);
        let error_message = planner_error_message(error);

        state.push_message(AgentMessage::custom(
            "planner.feedback.v1",
            json!({
                "stage": stage.as_str(),
                "error": error_message,
                "retry_count": retry_count,
                "retry_limit": self.max_parse_attempts,
                "retries_remaining": retries_remaining,
            }),
        ));

        if retry_count > self.max_parse_attempts {
            return PlannerRetryDecision::Stop {
                retry_count,
                reason: format!(
                    "planner {} failed after {} retries in turn {} step {}: {}",
                    stage.as_str(),
                    retry_count - 1,
                    state.turn_index,
                    state.step_index,
                    error_message
                ),
            };
        }

        PlannerRetryDecision::Retry {
            retry_count,
            retries_remaining,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepeatedErrorDecision {
    Continue,
    Stop {
        fingerprint: String,
        consecutive_error_count: u32,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepeatedErrorPolicy {
    repeated_error_limit: u32,
}

impl RepeatedErrorPolicy {
    pub fn new(repeated_error_limit: u32) -> Self {
        Self {
            repeated_error_limit,
        }
    }

    pub fn register_tool_result(
        &self,
        state: &mut AgentSessionState,
        result: &AgentToolResult,
    ) -> RepeatedErrorDecision {
        if !result.is_error {
            state.clear_error_tracking();
            return RepeatedErrorDecision::Continue;
        }

        let fingerprint = tool_error_fingerprint(result);
        let consecutive_error_count = state.record_error_fingerprint(fingerprint.clone());

        if consecutive_error_count > self.repeated_error_limit {
            return RepeatedErrorDecision::Stop {
                fingerprint: fingerprint.clone(),
                consecutive_error_count,
                reason: format!(
                    "tool failure loop detected: `{fingerprint}` repeated {} times (limit: {})",
                    consecutive_error_count, self.repeated_error_limit
                ),
            };
        }

        RepeatedErrorDecision::Continue
    }
}

fn planner_error_message(error: &AgentError) -> String {
    match error {
        AgentError::Planner(message)
        | AgentError::Config(message)
        | AgentError::ModelNotConfigured(message) => message.clone(),
        AgentError::Runtime(error) => error.to_string(),
        AgentError::Interrupted(message) => message.clone(),
    }
}

fn tool_error_fingerprint(result: &AgentToolResult) -> String {
    let kind = result
        .error
        .as_ref()
        .map(|error| error.kind.as_str())
        .unwrap_or("unknown");
    format!("{}:{kind}", result.tool_name)
}
