use operator_agent::{
    policy::{PlannerFailureStage, PlannerRetryDecision, PlannerRetryPolicy},
    session::AgentMessage,
    AgentError, AgentSessionState,
};
use operator_core::{SessionId, TargetId};
use serde_json::{json, Value};

fn sample_state() -> AgentSessionState {
    let mut state = AgentSessionState::new(
        SessionId("sess-parse".into()),
        TargetId("local:macos".into()),
        "Inspect the UI and click Save",
    );
    state.start_turn();
    state.start_step();
    state
}

fn custom_payload(message: &AgentMessage) -> (&str, &Value) {
    match message {
        AgentMessage::Custom { kind, payload } => (kind.as_ref(), payload),
        other => panic!("expected custom feedback message, got {other:?}"),
    }
}

#[test]
fn planner_retry_policy_records_feedback_and_remaining_budget() {
    let mut state = sample_state();
    let policy = PlannerRetryPolicy::new(2);

    let outcome = policy.register_failure(
        &mut state,
        PlannerFailureStage::Parse,
        &AgentError::Planner("planner output must be a JSON object".into()),
    );

    assert_eq!(
        outcome,
        PlannerRetryDecision::Retry {
            retry_count: 1,
            retries_remaining: 1,
        }
    );
    assert_eq!(state.parse_attempts, 1);
    assert_eq!(state.messages.len(), 1);

    let (kind, payload) = custom_payload(
        state
            .messages
            .last()
            .expect("feedback message should be recorded"),
    );
    assert_eq!(kind, "planner.feedback.v1");
    assert_eq!(
        payload,
        &json!({
            "stage": "parse",
            "error": "planner output must be a JSON object",
            "retry_count": 1,
            "retry_limit": 2,
            "retries_remaining": 1
        })
    );
}

#[test]
fn planner_retry_policy_stops_after_budget_is_exhausted() {
    let mut state = sample_state();
    let policy = PlannerRetryPolicy::new(1);
    let error = AgentError::Planner("planner tool `click` failed schema validation".into());

    let first = policy.register_failure(&mut state, PlannerFailureStage::Validation, &error);
    assert_eq!(
        first,
        PlannerRetryDecision::Retry {
            retry_count: 1,
            retries_remaining: 0,
        }
    );

    let second = policy.register_failure(&mut state, PlannerFailureStage::Validation, &error);
    let PlannerRetryDecision::Stop {
        retry_count,
        reason,
    } = second
    else {
        panic!("expected retry policy to stop after the limit is exceeded");
    };

    assert_eq!(retry_count, 2);
    assert!(
        reason.contains("validation"),
        "stop reason should mention the validation stage: {reason}"
    );
    assert!(
        reason.contains("planner tool `click` failed schema validation"),
        "stop reason should include the planner error: {reason}"
    );

    let (kind, payload) = custom_payload(
        state
            .messages
            .last()
            .expect("final feedback message should be recorded"),
    );
    assert_eq!(kind, "planner.feedback.v1");
    assert_eq!(payload["stage"], json!("validation"));
    assert_eq!(payload["retry_count"], json!(2));
    assert_eq!(payload["retries_remaining"], json!(0));
    assert_eq!(state.parse_attempts, 2);
}
