use std::sync::Arc;

use operator_agent::{
    planner::LoopStateContextManager,
    session::{AgentSessionState, VisualObservationSummary},
    tools::AgentToolResult,
};
use operator_core::{
    ArtifactId, Capability, CapabilitySet, ImageSizePx, SessionId, SnapshotId, TargetId,
};
use operator_runtime::{RuntimeBuilder, RuntimeConfig};
use operator_testkit::{test_element, test_snapshot, InMemorySnapshotStore, MockPlatformDriver};
use serde_json::json;

fn tool_result(
    tool_name: &str,
    output: Option<serde_json::Value>,
    is_error: bool,
    read_only: bool,
) -> AgentToolResult {
    AgentToolResult {
        tool_name: tool_name.into(),
        arguments: json!({}),
        output,
        error: None,
        is_error,
        read_only,
    }
}

fn observation(
    snapshot_id: &str,
    artifact_id: &str,
    root_element_count: usize,
    element_count: usize,
) -> VisualObservationSummary {
    VisualObservationSummary {
        snapshot_id: SnapshotId(snapshot_id.into()),
        surface: "frontmost".into(),
        screenshot_artifact: Some(ArtifactId(artifact_id.into())),
        image_size_px: Some(ImageSizePx {
            width: 1260,
            height: 2720,
        }),
        element_tree_reliability: None,
        element_tree_note: None,
        root_element_count,
        element_count,
        element_digest: None,
    }
}

#[tokio::test]
async fn planner_context_assembles_from_in_memory_visual_state_and_recent_tool_results() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::InspectTree,
            Capability::Capture,
            Capability::AppLifecycle,
        ]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");

    let mut snapshot = test_snapshot("snap-observe");
    let extra = test_element("el-2");
    snapshot.image_artifact = Some(ArtifactId("capture-observe.png".into()));
    snapshot
        .elements
        .get_mut(&"el-1".into())
        .expect("fixture root element should exist")
        .label = Some("Secret AX Tree Node".into());
    snapshot.elements.insert(extra.id.clone(), extra);

    let mut state = AgentSessionState::new(
        SessionId("sess-ctx".into()),
        TargetId("macos".into()),
        "Open Finder and confirm the window appears",
    );
    state.start_turn();
    state.start_step();
    state.record_visual_observation(observation("snap-before", "capture-before.png", 0, 0));
    state.record_visual_observation(observation("snap-latest", "capture-latest.png", 1, 2));
    state.latest_snapshot = Some("missing-snapshot".into());
    state.previous_snapshot_visual = Some(ArtifactId("stale-field.png".into()));
    state.push_tool_trace(
        tool_result("list-apps", Some(json!({ "apps": [] })), false, true),
        10,
    );
    state.push_tool_trace(
        tool_result(
            "observe",
            Some(json!({
                "snapshot": snapshot,
            })),
            false,
            true,
        ),
        11,
    );
    state.push_tool_trace(
        tool_result(
            "click",
            Some(json!({
                "outcome": "clicked"
            })),
            false,
            false,
        ),
        12,
    );
    state.push_tool_trace(
        tool_result(
            "list-windows",
            Some(json!({
                "windows": [{"title": "Finder"}]
            })),
            false,
            true,
        ),
        13,
    );
    state.push_tool_trace(
        tool_result(
            "artifact-get",
            Some(json!({
                "artifact": {
                    "id": "capture-latest.png",
                    "path": "/tmp/capture-latest.png"
                }
            })),
            false,
            true,
        ),
        14,
    );
    state.push_tool_trace(
        tool_result(
            "press",
            Some(json!({
                "keys": ["enter"]
            })),
            false,
            false,
        ),
        15,
    );
    state.add_note("Observe again before deciding the task is finished.");

    let context = LoopStateContextManager::new(runtime.core())
        .assemble(&state)
        .expect("context should assemble");

    assert_eq!(context.target.id, TargetId("macos".into()));
    assert_eq!(context.target.platform, "macos");
    assert!(context.include_elements);
    assert_eq!(
        context.target.capabilities,
        vec![
            "app_lifecycle".to_string(),
            "capture".to_string(),
            "inspect_tree".to_string(),
        ]
    );
    assert_eq!(context.recent_tool_results.len(), 5);
    assert_eq!(context.recent_tool_results[0].tool_name, "observe");
    assert!(
        context.recent_tool_results[0]
            .summary
            .contains("snap-observe"),
        "observe summary should retain the snapshot id"
    );
    assert!(
        !context.recent_tool_results[0]
            .summary
            .contains("Secret AX Tree Node"),
        "observe summary should not inline raw accessibility tree labels"
    );
    assert_eq!(
        context.current_observation,
        Some(observation("snap-latest", "capture-latest.png", 1, 2))
    );
    assert_eq!(
        context
            .current_observation
            .as_ref()
            .and_then(|observation| observation.image_size_px),
        Some(ImageSizePx {
            width: 1260,
            height: 2720,
        })
    );
    assert_eq!(
        context.current_visual_artifact,
        Some(ArtifactId("capture-latest.png".into()))
    );
    assert_eq!(
        context.previous_visual_artifact,
        Some(ArtifactId("capture-before.png".into()))
    );
    assert_eq!(context.notes.len(), 1);
    assert!(context.ui_state_stale);
}

#[tokio::test]
async fn planner_context_preserves_bounded_element_digest_from_snapshot_observation() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::InspectTree, Capability::Capture]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");

    let mut snapshot = test_snapshot("snap-digest");
    snapshot
        .elements
        .get_mut(&"el-1".into())
        .expect("fixture root element should exist")
        .bounds = Some(operator_core::Rect {
        x: 10.0,
        y: 20.0,
        width: 80.0,
        height: 30.0,
    });
    snapshot.image_artifact = Some(ArtifactId("capture-digest.png".into()));

    let mut state = AgentSessionState::new(
        SessionId("sess-digest".into()),
        TargetId("macos".into()),
        "Inspect the current UI",
    );
    state.record_observation_snapshot(&snapshot);

    let context = LoopStateContextManager::new(runtime.core())
        .assemble(&state)
        .expect("context should assemble");

    let digest = context
        .current_observation
        .as_ref()
        .and_then(|observation| observation.element_digest.as_ref())
        .expect("element digest should be present");
    assert_eq!(digest.entries.len(), 1);
    assert_eq!(digest.entries[0].element_id, "el-1");
    assert_eq!(
        digest.entries[0].label.as_deref(),
        Some("Test Element el-1")
    );
}

#[test]
fn successful_action_results_mark_ui_stale_until_a_successful_observe() {
    let mut state = AgentSessionState::new(
        SessionId("sess-stale".into()),
        TargetId("macos".into()),
        "Click Save",
    );
    state.start_turn();
    state.start_step();

    state.push_tool_trace(
        tool_result(
            "click",
            Some(json!({
                "outcome": "clicked"
            })),
            false,
            false,
        ),
        20,
    );
    assert!(
        state.ui_state_stale,
        "successful side-effect tools should mark the UI state stale"
    );

    state.push_tool_trace(
        AgentToolResult {
            tool_name: "observe".into(),
            arguments: json!({
                "include_elements": true,
                "surface": { "kind": "Frontmost" }
            }),
            output: Some(json!({
                "snapshot": {
                    "id": "snap-after",
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
        21,
    );
    assert!(
        !state.ui_state_stale,
        "successful observe tools should clear the stale UI marker"
    );
}
