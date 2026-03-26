use operator_agent::{
    model::{CallOptions, CoordinatePolicy, ModelConfig, ProviderKind},
    planner::{AgentDecision, DecisionNormalizer},
    session::AgentSessionState,
};
use operator_testkit::test_snapshot;
use serde_json::json;

fn model_config(policy: CoordinatePolicy) -> ModelConfig {
    ModelConfig {
        provider: ProviderKind::OpenAiCompatible,
        id: "test-model".into(),
        coordinate_policy: policy,
        default_options: CallOptions::default(),
        default_timeout_ms: Some(30_000),
    }
}

fn session_with_current_snapshot(snapshot_id: &str) -> AgentSessionState {
    let mut state = AgentSessionState::new("sess-1".into(), "local:macos".into(), "test");
    let snapshot = test_snapshot(snapshot_id);
    state.record_observation_snapshot(&snapshot);
    state
}

#[test]
fn normalizer_rewrites_surface_image_pixel_coordinates_against_current_snapshot() {
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "Coords": {
                    "x": 152.0,
                    "y": 772.0,
                }
            }
        }),
        summary: "Click the 1 button".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-1");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceImagePixels),
            &state,
        )
        .expect("surface image policy should rewrite image-pixel coordinates");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "click".into(),
            arguments: json!({
                "locator": {
                    "SnapshotPixelCoords": {
                        "snapshot": "snap-1",
                        "point": {
                            "x": 152.0,
                            "y": 772.0,
                        }
                    }
                }
            }),
            summary: "Click the 1 button".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_rewrites_surface_normalized_coordinates_against_current_snapshot() {
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "Coords": {
                    "x": 152.0,
                    "y": 772.0,
                }
            }
        }),
        summary: "Click the 1 button".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-current");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceNormalized1000),
            &state,
        )
        .expect("surface-normalized policy should rewrite coordinates");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "click".into(),
            arguments: json!({
                "locator": {
                    "SnapshotNormalizedCoords": {
                        "snapshot": "snap-current",
                        "point": {
                            "x": 152.0,
                            "y": 772.0,
                        },
                        "basis": 1000.0,
                    }
                }
            }),
            summary: "Click the 1 button".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_requires_current_snapshot_for_surface_relative_coordinates() {
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "Coords": {
                    "x": 152.0,
                    "y": 772.0,
                }
            }
        }),
        summary: "Click the 1 button".into(),
        thought: None,
    };
    let state = AgentSessionState::new("sess-1".into(), "local:macos".into(), "test");

    let error = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceAbsolutePixels),
            &state,
        )
        .expect_err("surface-relative coordinates need a current observation");

    assert!(
        error.to_string().contains("current observation snapshot"),
        "unexpected error: {error}"
    );
}

#[test]
fn normalizer_canonicalizes_snapshot_normalized_coords_to_surface_image_pixels() {
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "SnapshotNormalizedCoords": {
                    "snapshot": "older-snapshot",
                    "point": {
                        "x": 176.0,
                        "y": 314.0
                    },
                    "basis": 460.0
                }
            }
        }),
        summary: "Click the clear button".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-current");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceImagePixels),
            &state,
        )
        .expect("surface image policy should canonicalize coordinates");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "click".into(),
            arguments: json!({
                "locator": {
                    "SnapshotPixelCoords": {
                        "snapshot": "snap-current",
                        "point": {
                            "x": 176.0,
                            "y": 314.0,
                        }
                    }
                }
            }),
            summary: "Click the clear button".into(),
            thought: None,
        }
    );
}
