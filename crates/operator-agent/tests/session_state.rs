use operator_agent::model::{ContentBlock, Message, UserMessage};
use operator_agent::session::{
    AgentMessage, AgentSessionState, AgentSessionStatus, ToolTraceEntry,
};
use operator_agent::tools::{AgentToolError, AgentToolResult};
use operator_core::{ArtifactId, SessionId, SnapshotId, TargetId};
use serde_json::json;

fn sample_user_message() -> Message {
    Message::User(UserMessage {
        content: vec![ContentBlock::Text {
            text: "Open Finder.".into(),
        }],
        timestamp_ms: 7,
    })
}

fn sample_tool_result() -> AgentToolResult {
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
    }
}

#[test]
fn new_session_state_starts_with_clean_tracking_fields() {
    let state = AgentSessionState::new(
        SessionId("sess-1".into()),
        TargetId("local:macos".into()),
        "Open Finder and capture the window",
    );

    assert_eq!(state.session_id, SessionId("sess-1".into()));
    assert_eq!(state.target, TargetId("local:macos".into()));
    assert_eq!(state.task, "Open Finder and capture the window");
    assert_eq!(state.status, AgentSessionStatus::Running);
    assert_eq!(state.turn_index, 0);
    assert_eq!(state.step_index, 0);
    assert_eq!(state.parse_attempts, 0);
    assert!(state.messages.is_empty());
    assert!(state.tool_trace.is_empty());
    assert!(state.notes.is_empty());
    assert_eq!(state.latest_snapshot, None);
    assert_eq!(state.previous_snapshot_visual, None);
    assert!(state.latest_artifacts.is_empty());
    assert!(!state.ui_state_stale);
    assert_eq!(state.consecutive_error_count, 0);
    assert_eq!(state.last_error_fingerprint, None);
}

#[test]
fn agent_message_supports_model_and_custom_transcript_entries() {
    let base = AgentMessage::from(sample_user_message());
    assert_eq!(base.as_model_message(), Some(&sample_user_message()));

    let custom = AgentMessage::custom(
        "planner.note.v1",
        json!({
            "summary": "Need another observe before finishing."
        }),
    );
    assert_eq!(custom.as_model_message(), None);

    let round_trip = serde_json::from_value::<AgentMessage>(
        serde_json::to_value(&custom).expect("custom message should serialize"),
    )
    .expect("custom message should deserialize");
    assert_eq!(round_trip, custom);
}

#[test]
fn session_state_records_tool_trace_and_observation_updates() {
    let mut state = AgentSessionState::new(
        SessionId("sess-2".into()),
        TargetId("local:macos".into()),
        "Observe the frontmost window",
    );
    state.start_turn();
    state.start_step();
    state.push_message(sample_user_message());
    state.mark_ui_stale();

    let result = sample_tool_result();
    state.push_tool_trace(result.clone(), 99);
    state.record_observation(
        SnapshotId("snap-1".into()),
        vec![
            ArtifactId("capture-1.png".into()),
            ArtifactId("tree.json".into()),
        ],
        Some(ArtifactId("capture-1.png".into())),
    );

    assert_eq!(
        state.messages,
        vec![AgentMessage::from(sample_user_message())]
    );
    assert_eq!(
        state.tool_trace,
        vec![ToolTraceEntry {
            turn_index: 1,
            step_index: 1,
            timestamp_ms: 99,
            result,
        }]
    );
    assert_eq!(state.latest_snapshot, Some(SnapshotId("snap-1".into())));
    assert_eq!(
        state.latest_artifacts,
        vec![
            ArtifactId("capture-1.png".into()),
            ArtifactId("tree.json".into()),
        ]
    );
    assert_eq!(
        state.previous_snapshot_visual,
        Some(ArtifactId("capture-1.png".into()))
    );
    assert!(!state.ui_state_stale);
}

#[test]
fn screenshot_only_observe_keeps_ui_state_stale() {
    let mut state = AgentSessionState::new(
        SessionId("sess-2b".into()),
        TargetId("local:macos".into()),
        "Observe the frontmost window",
    );
    state.start_turn();
    state.start_step();
    state.mark_ui_stale();

    state.push_tool_trace(
        AgentToolResult {
            tool_name: "observe".into(),
            arguments: json!({
                "surface": { "kind": "Frontmost" },
                "include_screenshot": true
            }),
            output: Some(json!({
                "snapshot": {
                    "id": "snap-shot-only",
                    "surface": { "kind": "Frontmost" },
                    "root_ids": [],
                    "elements": {},
                    "image_artifact": "capture-shot-only.png"
                }
            })),
            error: None,
            is_error: false,
            read_only: true,
        },
        100,
    );

    assert!(
        state.ui_state_stale,
        "screenshot-only observe results should not clear stale UI state"
    );
}

#[test]
fn session_state_tracks_parse_attempts_notes_errors_and_terminal_status() {
    let mut state = AgentSessionState::new(
        SessionId("sess-3".into()),
        TargetId("local:macos".into()),
        "Click Save",
    );
    state.start_turn();
    state.start_step();

    assert_eq!(state.bump_parse_attempts(), 1);
    assert_eq!(state.bump_parse_attempts(), 2);

    state.add_note("Finish was rejected because Save was not confirmed.");
    assert_eq!(
        state.notes,
        vec!["Finish was rejected because Save was not confirmed."]
    );

    assert_eq!(state.record_error_fingerprint("click:tool"), 1);
    assert_eq!(state.record_error_fingerprint("click:tool"), 2);
    assert_eq!(state.record_error_fingerprint("observe:timeout"), 1);
    assert_eq!(
        state.last_error_fingerprint.as_deref(),
        Some("observe:timeout")
    );

    state.clear_error_tracking();
    assert_eq!(state.consecutive_error_count, 0);
    assert_eq!(state.last_error_fingerprint, None);

    state.start_step();
    assert_eq!(state.step_index, 2);
    assert_eq!(state.parse_attempts, 0);

    state.complete("Clicked Save and confirmed the dialog.");
    assert_eq!(
        state.status,
        AgentSessionStatus::Completed {
            summary: "Clicked Save and confirmed the dialog.".into(),
        }
    );

    state.fail("Model returned an unrecoverable parse error.");
    assert_eq!(
        state.status,
        AgentSessionStatus::Failed {
            reason: "Model returned an unrecoverable parse error.".into(),
        }
    );
}

#[test]
fn tool_trace_entries_keep_error_results_for_transcript_replay() {
    let result = AgentToolResult {
        tool_name: "click".into(),
        arguments: json!({ "x": 10, "y": 20 }),
        output: None,
        error: Some(AgentToolError {
            kind: "tool".into(),
            message: "tool error: click, message: side effects are disabled by runtime policy"
                .into(),
        }),
        is_error: true,
        read_only: false,
    };

    let entry = ToolTraceEntry {
        turn_index: 2,
        step_index: 4,
        timestamp_ms: 1234,
        result,
    };

    let round_trip = serde_json::from_value::<ToolTraceEntry>(
        serde_json::to_value(&entry).expect("tool trace entry should serialize"),
    )
    .expect("tool trace entry should deserialize");
    assert_eq!(round_trip, entry);
}
