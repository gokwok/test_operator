use operator_agent::planner::{AgentDecision, DecisionParser};

#[test]
fn parses_call_tool_json_and_preserves_thought() {
    let parser = DecisionParser::new();

    let decision = parser
        .parse(
            r#"{
                "decision": "call_tool",
                "name": "observe",
                "arguments": {
                    "surface": "frontmost"
                },
                "summary": "Inspect the current window before acting.",
                "thought": "Need fresh UI state first."
            }"#,
        )
        .expect("call_tool payload should parse");

    assert_eq!(
        decision,
        AgentDecision::CallTool {
            name: "observe".into(),
            arguments: serde_json::json!({
                "surface": "frontmost"
            }),
            summary: "Inspect the current window before acting.".into(),
            thought: Some("Need fresh UI state first.".into()),
        }
    );
}

#[test]
fn recovers_first_json_object_from_wrapped_planner_output() {
    let parser = DecisionParser::new();

    let decision = parser
        .parse(
            "I will return JSON only next time.\n```json\n{\"decision\":\"finish\",\"summary\":\"Finder is open and visible.\"}\n```",
        )
        .expect("wrapped JSON object should be recovered");

    assert_eq!(
        decision,
        AgentDecision::Finish {
            summary: "Finder is open and visible.".into(),
        }
    );
}

#[test]
fn rejects_unknown_decision_kinds() {
    let parser = DecisionParser::new();
    let error = parser
        .parse(
            r#"{
                "decision": "wait",
                "summary": "pause"
            }"#,
        )
        .expect_err("unknown decision kinds must be rejected");

    assert!(
        error.to_string().contains("unsupported planner decision"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_missing_required_fields() {
    let parser = DecisionParser::new();
    let error = parser
        .parse(
            r#"{
                "decision": "call_tool",
                "name": "observe",
                "summary": "Inspect the window."
            }"#,
        )
        .expect_err("missing arguments field must be rejected");

    assert!(
        error.to_string().contains("arguments"),
        "unexpected error: {error}"
    );
}
