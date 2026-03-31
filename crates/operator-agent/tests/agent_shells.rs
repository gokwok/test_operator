use operator_agent::{AgentConfig, AgentError, AgentRunRequest, AgentRunResult, PlannerFormat};
use operator_core::{OperatorError, SessionId, TargetId};

#[test]
fn agent_config_defaults_match_phase1_design() {
    let config = AgentConfig::default();

    assert_eq!(config.default_model, "openai");
    assert_eq!(config.max_steps, 40);
    assert_eq!(config.max_parse_attempts, 3);
    assert_eq!(config.repeated_error_limit, 3);
    assert_eq!(config.step_timeout_ms, 30_000);
    assert_eq!(config.planner_format, PlannerFormat::Json);
}

#[test]
fn request_and_result_shells_round_trip_through_json() {
    let request = AgentRunRequest {
        task: "Open Safari and focus the frontmost window".into(),
        target: TargetId("macos".into()),
        model: Some("openai".into()),
    };
    let result = AgentRunResult {
        session_id: SessionId("sess-1".into()),
        target: TargetId("macos".into()),
        model: "openai".into(),
        summary: "Opened Safari and focused the frontmost window.".into(),
    };

    let request_json = serde_json::to_value(&request).expect("request should serialize");
    let result_json = serde_json::to_value(&result).expect("result should serialize");

    assert_eq!(
        serde_json::from_value::<AgentRunRequest>(request_json)
            .expect("request should deserialize"),
        request
    );
    assert_eq!(
        serde_json::from_value::<AgentRunResult>(result_json).expect("result should deserialize"),
        result
    );
}

#[test]
fn agent_error_wraps_runtime_errors() {
    let error = AgentError::from(OperatorError::TargetBusy);

    assert!(format!("{error}").contains("runtime error"));
}
