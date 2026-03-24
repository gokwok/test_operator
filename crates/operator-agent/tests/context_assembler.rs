use std::sync::Arc;

use operator_agent::{
    planner::ContextAssembler, session::AgentSessionState, tools::AgentToolResult,
};
use operator_core::{ArtifactId, Capability, CapabilitySet, SessionId, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig, SnapshotStore};
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

#[tokio::test]
async fn assemble_compacts_target_snapshot_and_recent_tool_state() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::InspectTree,
            Capability::Capture,
            Capability::AppLifecycle,
        ]),
    ));
    let store = Arc::new(InMemorySnapshotStore::new());
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store.clone())
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");

    let mut snapshot = test_snapshot("snap-latest");
    let extra = test_element("el-2");
    snapshot.image_artifact = Some(ArtifactId("capture-latest.png".into()));
    snapshot
        .elements
        .get_mut(&"el-1".into())
        .expect("fixture root element should exist")
        .label = Some("Secret AX Tree Node".into());
    snapshot.elements.insert(extra.id.clone(), extra);
    store
        .save(&snapshot)
        .await
        .expect("snapshot should be saved");

    let mut state = AgentSessionState::new(
        SessionId("sess-ctx".into()),
        TargetId("local:macos".into()),
        "Open Finder and confirm the window appears",
    );
    state.start_turn();
    state.start_step();
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
    state.latest_snapshot = Some("snap-latest".into());
    state.previous_snapshot_visual = Some(ArtifactId("capture-before.png".into()));
    state.add_note("Observe again before deciding the task is finished.");

    let context = ContextAssembler::new(runtime.core())
        .assemble(&state)
        .await
        .expect("context should assemble");

    assert_eq!(context.target.id, TargetId("local:macos".into()));
    assert_eq!(context.target.platform, "macos");
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
            .contains("snap-latest"),
        "observe summary should retain the snapshot id"
    );
    assert!(
        !context.recent_tool_results[0]
            .summary
            .contains("Secret AX Tree Node"),
        "observe summary should not inline raw accessibility tree labels"
    );
    assert_eq!(
        context.previous_snapshot_visual,
        Some(ArtifactId("capture-before.png".into()))
    );
    assert_eq!(context.notes.len(), 1);
    assert!(context.ui_state_stale);

    let latest_snapshot = context
        .latest_snapshot
        .expect("latest snapshot summary should be present");
    assert_eq!(latest_snapshot.id, "snap-latest".into());
    assert_eq!(latest_snapshot.surface, "frontmost");
    assert_eq!(latest_snapshot.root_element_count, 1);
    assert_eq!(latest_snapshot.element_count, 2);
    assert_eq!(
        latest_snapshot.screenshot_artifact,
        Some(ArtifactId("capture-latest.png".into()))
    );
}

#[test]
fn successful_action_results_mark_ui_stale_until_a_successful_observe() {
    let mut state = AgentSessionState::new(
        SessionId("sess-stale".into()),
        TargetId("local:macos".into()),
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
        tool_result(
            "observe",
            Some(json!({
                "snapshot": {
                    "id": "snap-after"
                }
            })),
            false,
            true,
        ),
        21,
    );
    assert!(
        !state.ui_state_stale,
        "successful observe tools should clear the stale UI marker"
    );
}
