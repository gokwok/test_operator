use operator_agent::{
    policy::{RepeatedErrorDecision, RepeatedErrorPolicy},
    tools::{AgentToolError, AgentToolResult},
    AgentSessionState,
};
use operator_core::{SessionId, TargetId};
use serde_json::json;

fn sample_state() -> AgentSessionState {
    let mut state = AgentSessionState::new(
        SessionId("sess-errors".into()),
        TargetId("local:macos".into()),
        "Click Save",
    );
    state.start_turn();
    state.start_step();
    state
}

fn tool_error(tool_name: &str, kind: &str, message: &str) -> AgentToolResult {
    AgentToolResult {
        tool_name: tool_name.into(),
        arguments: json!({}),
        output: None,
        error: Some(AgentToolError {
            kind: kind.into(),
            message: message.into(),
        }),
        is_error: true,
        read_only: false,
    }
}

fn tool_success(tool_name: &str) -> AgentToolResult {
    AgentToolResult {
        tool_name: tool_name.into(),
        arguments: json!({}),
        output: Some(json!({ "ok": true })),
        error: None,
        is_error: false,
        read_only: false,
    }
}

#[test]
fn repeated_error_policy_tracks_fingerprints_by_tool_name_and_error_kind() {
    let mut state = sample_state();
    let policy = RepeatedErrorPolicy::new(3);

    let first = policy.register_tool_result(
        &mut state,
        &tool_error("click", "tool", "button is disabled"),
    );
    assert_eq!(first, RepeatedErrorDecision::Continue);
    assert_eq!(state.consecutive_error_count, 1);
    assert_eq!(state.last_error_fingerprint.as_deref(), Some("click:tool"));

    let second =
        policy.register_tool_result(&mut state, &tool_error("click", "tool", "still disabled"));
    assert_eq!(second, RepeatedErrorDecision::Continue);
    assert_eq!(state.consecutive_error_count, 2);
    assert_eq!(state.last_error_fingerprint.as_deref(), Some("click:tool"));

    let third = policy.register_tool_result(
        &mut state,
        &tool_error("observe", "timeout", "capture timed out"),
    );
    assert_eq!(third, RepeatedErrorDecision::Continue);
    assert_eq!(state.consecutive_error_count, 1);
    assert_eq!(
        state.last_error_fingerprint.as_deref(),
        Some("observe:timeout")
    );
}

#[test]
fn repeated_error_policy_stops_past_limit_and_clears_after_success() {
    let mut state = sample_state();
    let policy = RepeatedErrorPolicy::new(2);
    let repeated_error = tool_error("click", "tool", "button is disabled");

    assert_eq!(
        policy.register_tool_result(&mut state, &repeated_error),
        RepeatedErrorDecision::Continue
    );
    assert_eq!(
        policy.register_tool_result(&mut state, &repeated_error),
        RepeatedErrorDecision::Continue
    );

    let stop = policy.register_tool_result(&mut state, &repeated_error);
    let RepeatedErrorDecision::Stop {
        fingerprint,
        consecutive_error_count,
        reason,
    } = stop
    else {
        panic!("expected repeated error policy to stop after the threshold is exceeded");
    };
    assert_eq!(fingerprint, "click:tool");
    assert_eq!(consecutive_error_count, 3);
    assert!(
        reason.contains("click:tool"),
        "stop reason should mention the repeated fingerprint: {reason}"
    );

    assert_eq!(
        policy.register_tool_result(&mut state, &tool_success("observe")),
        RepeatedErrorDecision::Continue
    );
    assert_eq!(state.consecutive_error_count, 0);
    assert_eq!(state.last_error_fingerprint, None);

    assert_eq!(
        policy.register_tool_result(&mut state, &repeated_error),
        RepeatedErrorDecision::Continue
    );
    assert_eq!(state.consecutive_error_count, 1);
    assert_eq!(state.last_error_fingerprint.as_deref(), Some("click:tool"));
}
