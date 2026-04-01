mod support;

use std::sync::Arc;

use operator_agent::{
    model::{
        ContentBlock, Context, Message, ModelRegistry, ProviderKind, ResolvedModel, UserMessage,
    },
    planner::{FinishGate, FinishGateVerdict},
    session::VisualObservationSummary,
    tools::AgentToolResult,
    AgentSessionState,
};
use operator_core::{ArtifactId, SessionId, SnapshotId, TargetId};
use serde_json::{json, Value};
use support::DeterministicTestProvider;

fn finish_gate_request_json(context: &Context) -> Value {
    let Some(Message::User(UserMessage { content, .. })) = context.messages.last() else {
        panic!("finish gate request should append a final user message");
    };
    let text = content
        .iter()
        .find_map(|block| match block {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .expect("finish gate request should contain a text block");

    serde_json::from_str(text).expect("finish gate request payload should be valid json")
}

fn resolved_model(
    provider_kind: ProviderKind,
    text: &str,
) -> (ResolvedModel, Arc<DeterministicTestProvider>) {
    let provider = Arc::new(DeterministicTestProvider::new(text));
    let mut registry = ModelRegistry::new();
    registry.register_provider(provider_kind, provider.clone());
    let model_name = match provider_kind {
        ProviderKind::OpenAi => "gpt-5.4",
        ProviderKind::OpenAiCompatible => "doubao-seed",
    };

    (
        registry
            .resolve(model_name)
            .expect("test model should resolve"),
        provider,
    )
}

fn verified_read_only_state() -> AgentSessionState {
    let mut state = AgentSessionState::new(
        SessionId("sess-finish-gate".into()),
        TargetId("macos".into()),
        "Open Finder and confirm the window appears",
    );
    state.start_turn();
    state.start_step();
    state.push_tool_trace(
        AgentToolResult {
            tool_name: "observe".into(),
            arguments: json!({
                "surface": { "kind": "Frontmost" },
                "include_elements": true,
                "include_screenshot": false,
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
    state.record_visual_observation(VisualObservationSummary {
        snapshot_id: SnapshotId("snap-1".into()),
        surface: "frontmost".into(),
        screenshot_artifact: Some(ArtifactId("capture-1.png".into())),
        image_size_px: None,
        root_element_count: 1,
        element_count: 1,
        element_digest: None,
    });
    state
}

fn verified_side_effect_state() -> AgentSessionState {
    let mut state = verified_read_only_state();
    state.push_tool_trace(
        AgentToolResult {
            tool_name: "click".into(),
            arguments: json!({}),
            output: Some(json!({ "success": true })),
            error: None,
            is_error: false,
            read_only: false,
        },
        1,
    );
    state.push_tool_trace(
        AgentToolResult {
            tool_name: "observe".into(),
            arguments: json!({
                "surface": { "kind": "Frontmost" },
                "include_elements": true,
                "include_screenshot": false,
            }),
            output: Some(json!({
                "snapshot": {
                    "id": "snap-2",
                    "root_ids": ["ax-1"],
                    "elements": {
                        "ax-1": {
                            "id": "ax-1",
                            "role": "AXWindow"
                        }
                    }
                }
            })),
            error: None,
            is_error: false,
            read_only: true,
        },
        2,
    );
    state.record_visual_observation(VisualObservationSummary {
        snapshot_id: SnapshotId("snap-2".into()),
        surface: "frontmost".into(),
        screenshot_artifact: Some(ArtifactId("capture-2.png".into())),
        image_size_px: None,
        root_element_count: 1,
        element_count: 1,
        element_digest: None,
    });
    state
}

fn verified_screenshot_only_state() -> AgentSessionState {
    let mut state = AgentSessionState::new(
        SessionId("sess-finish-gate-shot-only".into()),
        TargetId("macos".into()),
        "Confirm the UI from a screenshot",
    );
    state.set_include_elements(false);
    state.start_turn();
    state.start_step();
    state.push_tool_trace(
        AgentToolResult {
            tool_name: "observe".into(),
            arguments: json!({
                "surface": { "kind": "Frontmost" },
                "include_elements": false,
                "include_screenshot": true,
            }),
            output: Some(json!({
                "snapshot": {
                    "id": "snap-shot-only",
                    "root_ids": [],
                    "elements": {},
                    "image_artifact": "capture-shot-only.png"
                }
            })),
            error: None,
            is_error: false,
            read_only: true,
        },
        3,
    );
    state.record_visual_observation(VisualObservationSummary {
        snapshot_id: SnapshotId("snap-shot-only".into()),
        surface: "frontmost".into(),
        screenshot_artifact: Some(ArtifactId("capture-shot-only.png".into())),
        image_size_px: None,
        root_element_count: 0,
        element_count: 0,
        element_digest: None,
    });
    state
}

#[tokio::test]
async fn finish_gate_accepts_verified_read_only_finish_without_model_reflection() {
    let finish_gate = FinishGate::new();
    let state = verified_read_only_state();
    let (model, provider) = resolved_model(
        ProviderKind::OpenAi,
        r#"{"verdict":"not_ok","reason":"This should never be used."}"#,
    );

    let verdict = finish_gate
        .evaluate(&model, &state, "Finder is open and visible.")
        .await
        .expect("deterministic ok verdict should succeed");

    assert_eq!(
        verdict,
        FinishGateVerdict::Ok {
            reason: "The task has a fresh usable observe result and no recent side-effect actions require extra finish reflection.".into(),
        }
    );
    assert!(
        provider.requests().is_empty(),
        "deterministic finish acceptance should not call the model"
    );
}

#[tokio::test]
async fn finish_gate_accepts_screenshot_only_finish_when_include_elements_is_disabled() {
    let finish_gate = FinishGate::new();
    let state = verified_screenshot_only_state();
    let (model, provider) = resolved_model(
        ProviderKind::OpenAi,
        r#"{"verdict":"not_ok","reason":"This should never be used."}"#,
    );

    let verdict = finish_gate
        .evaluate(&model, &state, "The screenshot confirms the UI.")
        .await
        .expect("deterministic ok verdict should succeed");

    assert_eq!(
        verdict,
        FinishGateVerdict::Ok {
            reason: "The task has a fresh usable observe result and no recent side-effect actions require extra finish reflection.".into(),
        }
    );
    assert!(
        provider.requests().is_empty(),
        "screenshot-only finish acceptance should not call the model"
    );
}

#[tokio::test]
async fn finish_gate_recovers_wrapped_not_ok_verdicts_for_recent_side_effects() {
    let finish_gate = FinishGate::new();
    let state = verified_side_effect_state();
    let (model, provider) = resolved_model(
        ProviderKind::OpenAi,
        "Need one more check.\n```json\n{\"verdict\":\"not_ok\",\"reason\":\"The recent click still needs a stronger confirmation before finishing.\"}\n```",
    );

    let verdict = finish_gate
        .evaluate(&model, &state, "Finder is open and visible.")
        .await
        .expect("wrapped not_ok verdict should parse");

    assert_eq!(
        verdict,
        FinishGateVerdict::NotOk {
            reason: "The recent click still needs a stronger confirmation before finishing.".into(),
        }
    );

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        1,
        "recent side effects should force reflection"
    );
    let request = finish_gate_request_json(&requests[0].context);
    assert_eq!(
        request["current_visual_artifact"],
        Value::String("capture-2.png".into())
    );
    assert_eq!(
        request["previous_visual_artifact"],
        Value::String("capture-1.png".into())
    );
    assert!(request.get("transcript").is_none());
    assert!(request.get("tool_trace").is_none());
    assert!(
        request["recent_history"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "recent history should be present in the lightweight finish-gate context: {request}"
    );
}

#[tokio::test]
async fn finish_gate_rejects_invalid_verdict_payloads() {
    let finish_gate = FinishGate::new();
    let state = verified_side_effect_state();
    let (model, _) = resolved_model(
        ProviderKind::OpenAi,
        r#"{"verdict":"maybe","reason":"unclear"}"#,
    );

    let error = finish_gate
        .evaluate(&model, &state, "Finder is open and visible.")
        .await
        .expect_err("invalid verdicts must be rejected");

    assert!(
        error.to_string().contains("finish gate verdict"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn finish_gate_rejects_finish_when_ui_state_is_stale_without_model_reflection() {
    let finish_gate = FinishGate::new();
    let mut state = verified_read_only_state();
    state.mark_ui_stale();

    let (model, provider) = resolved_model(
        ProviderKind::OpenAiCompatible,
        r#"{"verdict":"ok","reason":"This should never be used."}"#,
    );

    let verdict = finish_gate
        .evaluate(&model, &state, "The task is definitely complete.")
        .await
        .expect("stale ui should be rejected before model reflection");

    assert_eq!(
        verdict,
        FinishGateVerdict::NotOk {
            reason:
                "The task is not verified yet because there is no fresh usable observe result after the last UI change."
                    .into(),
        }
    );
    assert!(
        provider.requests().is_empty(),
        "deterministic stale-ui rejection should not call the model"
    );
}

#[tokio::test]
async fn finish_gate_rejects_finish_without_a_usable_observe_result() {
    let finish_gate = FinishGate::new();
    let state = AgentSessionState::new(
        SessionId("sess-finish-gate-missing".into()),
        TargetId("macos".into()),
        "Confirm the desktop state",
    );
    let (model, provider) = resolved_model(
        ProviderKind::OpenAi,
        r#"{"verdict":"ok","reason":"This should never be used."}"#,
    );

    let verdict = finish_gate
        .evaluate(&model, &state, "Everything is done.")
        .await
        .expect("missing observe proof should be rejected deterministically");

    assert_eq!(
        verdict,
        FinishGateVerdict::NotOk {
            reason:
                "The task is not verified yet because there is no usable observe result in the current loop state."
                    .into(),
        }
    );
    assert!(
        provider.requests().is_empty(),
        "missing observe proof should not call the model"
    );
}
