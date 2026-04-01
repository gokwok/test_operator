#[allow(dead_code)]
#[path = "../examples/local_run.rs"]
mod local_run_example;

use std::{path::PathBuf, process::Command, time::SystemTime};

use clap::Parser;
use operator_agent::{
    render_harness_report, summarize_timing, summarize_transcript_replay, AgentRunRequest,
    AgentRunResult, HarnessReport, PersistedSessionTranscript, ReplayableTranscriptEvent,
};
use operator_bootstrap::runtime_config_path;
use operator_core::{ArtifactId, SessionId, TargetId};
use operator_runtime::{Session, SessionStatus};
use operator_testkit::test_snapshot;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn local_run_help_describes_the_developer_harness_surface() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate lives under workspace root")
        .to_path_buf();

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "operator-agent",
            "--example",
            "local_run",
            "--",
            "--help",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("cargo run --example local_run -- --help should run");

    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("developer-only local agent harness"),
        "help output should mark the example as developer-only: {stdout}"
    );
    assert!(
        stdout.contains("--task"),
        "help output should expose --task: {stdout}"
    );
    assert!(
        stdout.contains("--target"),
        "help output should expose --target: {stdout}"
    );
    assert!(
        stdout.contains("for example macos"),
        "help output should describe named targets: {stdout}"
    );
    assert!(
        stdout.contains("--model"),
        "help output should expose --model: {stdout}"
    );
}

#[test]
fn local_run_runtime_config_uses_operator_home_targets_and_allows_target_override() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(
        runtime_config_path(temp.path()),
        r#"
[runtime]
default_target = "windows-lab"

[targets.windows-lab]
platform = "windows"
driver = "windows.remote"

[targets.windows-lab.driver_config]
endpoint = "wss://windows-lab.internal"
"#,
    )
    .expect("write config");

    let config = local_run_example::runtime_config_for_home(
        &local_run_example::Cli::parse_from([
            "local_run",
            "--task",
            "Summarize the window",
            "--model",
            "gpt-5.4",
        ]),
        temp.path(),
    )
    .expect("load config");
    assert_eq!(config.default_target, TargetId("windows-lab".into()));

    let overridden = local_run_example::runtime_config_for_home(
        &local_run_example::Cli::parse_from([
            "local_run",
            "--task",
            "Summarize the window",
            "--target",
            "harmony-phone",
            "--model",
            "gpt-5.4",
        ]),
        temp.path(),
    )
    .expect("load overridden config");
    assert_eq!(overridden.default_target, TargetId("harmony-phone".into()));
}

#[test]
fn local_harness_report_surfaces_visual_references_and_timing_summaries() {
    let mut first_snapshot = test_snapshot("snap-before");
    first_snapshot.image_artifact = Some(ArtifactId("capture-before.png".into()));
    first_snapshot.metadata.capture_duration_ms = 4;

    let mut second_snapshot = test_snapshot("snap-after");
    second_snapshot.image_artifact = Some(ArtifactId("capture-after.png".into()));
    second_snapshot.metadata.capture_duration_ms = 7;

    let transcript = PersistedSessionTranscript {
        session: Session {
            id: SessionId("session-42".into()),
            created_at: SystemTime::UNIX_EPOCH,
            task: "Click Save and verify the UI.".into(),
            status: SessionStatus::Running,
        },
        events: vec![
            ReplayableTranscriptEvent::UserInput {
                text: "Click Save and verify the UI.".into(),
            },
            ReplayableTranscriptEvent::ToolCall {
                name: "observe".into(),
                input: json!({"include_screenshot": true}),
            },
            ReplayableTranscriptEvent::ToolResult {
                result: operator_agent::tools::AgentToolResult {
                    tool_name: "observe".into(),
                    arguments: json!({"include_screenshot": true, "include_elements": false}),
                    output: Some(json!({"snapshot": first_snapshot})),
                    error: None,
                    is_error: false,
                    read_only: true,
                },
            },
            ReplayableTranscriptEvent::ToolCall {
                name: "click".into(),
                input: json!({}),
            },
            ReplayableTranscriptEvent::ToolResult {
                result: operator_agent::tools::AgentToolResult {
                    tool_name: "click".into(),
                    arguments: json!({}),
                    output: Some(
                        json!({"success": true, "duration_ms": 12, "detail": "clicked Save"}),
                    ),
                    error: None,
                    is_error: false,
                    read_only: false,
                },
            },
            ReplayableTranscriptEvent::ToolCall {
                name: "observe".into(),
                input: json!({"include_elements": true}),
            },
            ReplayableTranscriptEvent::ToolResult {
                result: operator_agent::tools::AgentToolResult {
                    tool_name: "observe".into(),
                    arguments: json!({"include_screenshot": false, "include_elements": true}),
                    output: Some(json!({"snapshot": second_snapshot})),
                    error: None,
                    is_error: false,
                    read_only: true,
                },
            },
            ReplayableTranscriptEvent::Completed {
                summary: Some("Verified Save completed.".into()),
            },
        ],
    };

    let replay = summarize_transcript_replay(Some(&transcript)).expect("summary should exist");
    assert_eq!(replay.replayable_event_count, 8);
    assert_eq!(replay.observation_count, 2);
    assert_eq!(
        replay.current_visual_artifact,
        Some(ArtifactId("capture-after.png".into()))
    );
    assert_eq!(
        replay.previous_visual_artifact,
        Some(ArtifactId("capture-before.png".into()))
    );

    let timing = summarize_timing(Some(&transcript)).expect("timing should exist");
    assert_eq!(timing.measurement_count, 3);
    assert_eq!(timing.total_duration_ms, 23);
    assert_eq!(timing.by_tool.len(), 2);
    assert!(
        timing.by_tool.iter().any(|entry| {
            entry.tool_name == "click"
                && entry.measurement_kind == "duration_ms"
                && entry.total_duration_ms == 12
        }),
        "click duration summary should be preserved: {timing:?}"
    );
    assert!(
        timing.by_tool.iter().any(|entry| {
            entry.tool_name == "observe"
                && entry.measurement_kind == "capture_duration_ms"
                && entry.total_duration_ms == 11
        }),
        "observe capture summary should be preserved: {timing:?}"
    );

    let report = HarnessReport::new(
        AgentRunRequest {
            task: "Click Save and verify the UI.".into(),
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
            app: None,
        },
        PathBuf::from("/tmp/operator-agent-harness"),
        Some(AgentRunResult {
            session_id: SessionId("session-42".into()),
            target: TargetId("macos".into()),
            model: "gpt-5.4".into(),
            summary: "Verified Save completed.".into(),
        }),
        None,
        Some(transcript),
    );
    let rendered = render_harness_report(&report);

    assert!(
        rendered.contains("== Replay Summary =="),
        "report should render replay section: {rendered}"
    );
    assert!(
        rendered.contains("current_visual_artifact: capture-after.png"),
        "report should surface the latest visual artifact: {rendered}"
    );
    assert!(
        rendered.contains("previous_visual_artifact: capture-before.png"),
        "report should surface the previous visual artifact: {rendered}"
    );
    assert!(
        rendered.contains("== Timing Summary =="),
        "report should render timing section: {rendered}"
    );
    assert!(
        rendered.contains("click duration_ms count=1 total_ms=12"),
        "report should render action timing totals: {rendered}"
    );
    assert!(
        rendered.contains("observe capture_duration_ms count=2 total_ms=11"),
        "report should render observe capture totals: {rendered}"
    );
}
