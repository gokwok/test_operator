use operator_agent::{
    model::{CallOptions, CoordinatePolicy, ModelConfig, ProviderKind},
    planner::{AgentDecision, DecisionNormalizer},
    session::AgentSessionState,
};
use operator_core::SurfaceKind;
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
    let mut state = AgentSessionState::new("sess-1".into(), "macos".into(), "test");
    let snapshot = test_snapshot(snapshot_id);
    state.record_observation_snapshot(&snapshot);
    state
}

fn session_with_window_snapshot(snapshot_id: &str) -> AgentSessionState {
    let mut state = AgentSessionState::new("sess-1".into(), "macos".into(), "test");
    let mut snapshot = test_snapshot(snapshot_id);
    snapshot.surface.kind = SurfaceKind::Window { id: 42.into() };
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
            true,
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
            true,
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
    let state = AgentSessionState::new("sess-1".into(), "macos".into(), "test");

    let error = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceAbsolutePixels),
            &state,
            true,
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
            true,
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

#[test]
fn normalizer_forces_observe_include_elements_off_when_disabled() {
    let decision = AgentDecision::CallTool {
        name: "observe".into(),
        arguments: json!({
            "surface": { "kind": "Frontmost" },
            "include_elements": true,
            "include_screenshot": true
        }),
        summary: "Verify the current UI.".into(),
        thought: None,
    };
    let state = AgentSessionState::new("sess-1".into(), "macos".into(), "test");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::ScreenAbsolutePixels),
            &state,
            false,
        )
        .expect("observe normalization should succeed");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "observe".into(),
            arguments: json!({
                "surface": { "kind": "Frontmost" },
                "include_elements": false,
                "include_screenshot": true
            }),
            summary: "Verify the current UI.".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_drops_frontmost_app_selector_and_verifications_for_direct_type() {
    let decision = AgentDecision::CallTool {
        name: "type".into(),
        arguments: json!({
            "text": "777*999=",
            "target_selector": {
                "App": "Calculator"
            },
            "verifications": ["Focus", "WindowState"]
        }),
        summary: "Type the expression into Calculator.".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-frontmost");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::ScreenAbsolutePixels),
            &state,
            false,
        )
        .expect("frontmost direct type should normalize");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "type".into(),
            arguments: json!({
                "text": "777*999="
            }),
            summary: "Type the expression into Calculator.".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_drops_verifications_for_frontmost_click_without_selector() {
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "SnapshotPixelCoords": {
                    "snapshot": "older-snapshot",
                    "point": {
                        "x": 176.0,
                        "y": 314.0
                    }
                }
            },
            "verifications": ["Focus", "WindowState", "Geometry"]
        }),
        summary: "Click the clear button.".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-current");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceImagePixels),
            &state,
            false,
        )
        .expect("frontmost direct click should normalize");

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
            summary: "Click the clear button.".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_drops_null_selector_and_verifications_for_frontmost_click() {
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "SnapshotPixelCoords": {
                    "snapshot": "older-snapshot",
                    "point": {
                        "x": 469.0,
                        "y": 1613.0
                    }
                }
            },
            "target_selector": null,
            "verifications": ["Geometry"]
        }),
        summary: "Click the add button.".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-current");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::SurfaceImagePixels),
            &state,
            false,
        )
        .expect("frontmost direct click should drop null selectors");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "click".into(),
            arguments: json!({
                "locator": {
                    "SnapshotPixelCoords": {
                        "snapshot": "snap-current",
                        "point": {
                            "x": 469.0,
                            "y": 1613.0,
                        }
                    }
                }
            }),
            summary: "Click the add button.".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_drops_null_selector_and_verifications_for_frontmost_type() {
    let decision = AgentDecision::CallTool {
        name: "type".into(),
        arguments: json!({
            "text": "hello",
            "target_selector": null,
            "verifications": ["Focus", "WindowState"]
        }),
        summary: "Type into the frontmost app.".into(),
        thought: None,
    };
    let state = session_with_current_snapshot("snap-current");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::ScreenAbsolutePixels),
            &state,
            false,
        )
        .expect("frontmost direct type should drop null selectors");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "type".into(),
            arguments: json!({
                "text": "hello"
            }),
            summary: "Type into the frontmost app.".into(),
            thought: None,
        }
    );
}

#[test]
fn normalizer_preserves_app_selector_for_non_frontmost_observation() {
    let decision = AgentDecision::CallTool {
        name: "type".into(),
        arguments: json!({
            "text": "hello",
            "target_selector": {
                "App": "Notes"
            },
            "verifications": ["Focus"]
        }),
        summary: "Type into Notes.".into(),
        thought: None,
    };
    let state = session_with_window_snapshot("snap-window");

    let normalized = DecisionNormalizer::new()
        .normalize(
            decision,
            &model_config(CoordinatePolicy::ScreenAbsolutePixels),
            &state,
            false,
        )
        .expect("non-frontmost observations should preserve explicit app targeting");

    assert_eq!(
        normalized,
        AgentDecision::CallTool {
            name: "type".into(),
            arguments: json!({
                "text": "hello",
                "target_selector": {
                    "App": "Notes"
                },
                "verifications": ["Focus"]
            }),
            summary: "Type into Notes.".into(),
            thought: None,
        }
    );
}
