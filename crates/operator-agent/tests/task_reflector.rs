mod support;

use std::sync::Arc;

use operator_agent::{
    model::{
        AssistantMessage, ContentBlock, Message, ModelRegistry, ProviderKind, ResolvedModel,
        StopReason, Usage, UserMessage,
    },
    planner::{TaskReflection, TaskReflector},
    session::AgentMessage,
    tools::AgentToolResult,
    AgentSessionState,
};
use operator_core::{SessionId, TargetId};
use serde_json::json;
use support::DeterministicTestProvider;

fn sample_state() -> AgentSessionState {
    let mut state = AgentSessionState::new(
        SessionId("sess-reflect".into()),
        TargetId("local:macos".into()),
        "Open Finder and confirm the window appears",
    );
    state.start_turn();
    state.start_step();
    state.push_message(Message::User(UserMessage {
        content: vec![ContentBlock::Text {
            text: "Open Finder.".into(),
        }],
        timestamp_ms: 1,
    }));
    state.push_message(AgentMessage::from(Message::Assistant(AssistantMessage {
        content: vec![ContentBlock::Text {
            text: "I opened Finder and observed the desktop.".into(),
        }],
        usage: Usage::default(),
        stop: StopReason::Stop,
        error_message: None,
        timestamp_ms: 2,
    })));
    state.push_tool_trace(
        AgentToolResult {
            tool_name: "observe".into(),
            arguments: json!({
                "surface": { "kind": "Frontmost" },
                "include_elements": true
            }),
            output: Some(json!({
                "snapshot": {
                    "id": "snap-1",
                    "root_ids": ["ax-0"],
                    "elements": {
                        "ax-0": {
                            "id": "ax-0",
                            "role": "AXWindow"
                        }
                    }
                }
            })),
            error: None,
            is_error: false,
            read_only: true,
        },
        3,
    );
    state
}

fn resolved_model(text: &str) -> ResolvedModel {
    let mut registry = ModelRegistry::new();
    registry.register_provider(
        ProviderKind::OpenAi,
        Arc::new(DeterministicTestProvider::new(text)),
    );
    registry
        .resolve("gpt-5.4")
        .expect("test model should resolve")
}

#[tokio::test]
async fn task_reflector_accepts_ok_verdicts() {
    let reflector = TaskReflector::new();
    let state = sample_state();
    let model = resolved_model(
        r#"{"verdict":"ok","reason":"The transcript shows Finder was opened and the observe result confirmed the task."}"#,
    );

    let verdict = reflector
        .reflect(&model, &state, "Finder is open and visible.")
        .await
        .expect("ok verdict should parse");

    assert_eq!(
        verdict,
        TaskReflection::Ok {
            reason:
                "The transcript shows Finder was opened and the observe result confirmed the task."
                    .into(),
        }
    );
}

#[tokio::test]
async fn task_reflector_recovers_wrapped_not_ok_verdicts() {
    let reflector = TaskReflector::new();
    let state = sample_state();
    let model = resolved_model(
        "Need one more check.\n```json\n{\"verdict\":\"not_ok\",\"reason\":\"The finish summary claims success, but the transcript never confirms the Finder window became frontmost.\"}\n```",
    );

    let verdict = reflector
        .reflect(&model, &state, "Finder is open and visible.")
        .await
        .expect("wrapped not_ok verdict should parse");

    assert_eq!(
        verdict,
        TaskReflection::NotOk {
            reason: "The finish summary claims success, but the transcript never confirms the Finder window became frontmost."
                .into(),
        }
    );
}

#[tokio::test]
async fn task_reflector_rejects_invalid_verdict_payloads() {
    let reflector = TaskReflector::new();
    let state = sample_state();
    let model = resolved_model(r#"{"verdict":"maybe","reason":"unclear"}"#);

    let error = reflector
        .reflect(&model, &state, "Finder is open and visible.")
        .await
        .expect_err("invalid verdicts must be rejected");

    assert!(
        error.to_string().contains("reflector verdict"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn task_reflector_rejects_finish_when_ui_state_is_stale() {
    let reflector = TaskReflector::new();
    let mut state = sample_state();
    state.mark_ui_stale();

    let provider = Arc::new(DeterministicTestProvider::new(
        r#"{"verdict":"ok","reason":"This should never be used."}"#,
    ));
    let mut registry = ModelRegistry::new();
    registry.register_provider(ProviderKind::OpenAiCompatible, provider.clone());
    let model = registry
        .resolve("doubao-seed")
        .expect("doubao test model should resolve");

    let verdict = reflector
        .reflect(&model, &state, "The task is definitely complete.")
        .await
        .expect("stale ui should be rejected before model reflection");

    assert_eq!(
        verdict,
        TaskReflection::NotOk {
            reason:
                "The task is not verified yet because there is no fresh usable observe result after the last UI change."
                    .into(),
        }
    );
    assert!(
        provider.requests().is_empty(),
        "deterministic stale-ui rejection should not call the reflector model"
    );
}
