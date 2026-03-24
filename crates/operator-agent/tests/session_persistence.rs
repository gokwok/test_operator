mod support;

use std::sync::Arc;

use operator_agent::{
    model::{ModelRegistry, ProviderKind},
    session::{load_persisted_session, ReplayableTranscriptEvent},
    AgentConfig, AgentError, AgentRunRequest, AgentRunner,
};
use operator_core::{Capability, CapabilitySet, OperatorError, TargetId};
use operator_runtime::{
    FileSessionStore, RuntimeBuilder, RuntimeConfig, SessionStatus, SessionStore,
};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};
use tempfile::tempdir;

use support::DeterministicTestProvider;

async fn runner_with(
    driver: Arc<MockPlatformDriver>,
    provider: Arc<DeterministicTestProvider>,
    session_root: &std::path::Path,
    config: AgentConfig,
) -> AgentRunner {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .session_store(Arc::new(FileSessionStore::new(session_root)))
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");

    let mut models = ModelRegistry::new();
    models.register_provider(ProviderKind::OpenAi, provider);

    AgentRunner::new(Arc::new(runtime), models, config)
}

#[tokio::test]
async fn file_session_store_loads_deterministic_replayable_transcripts() {
    let dir = tempdir().unwrap();
    let driver = Arc::new(MockPlatformDriver::new("macos", CapabilitySet::new([])));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"finish","summary":"Bootstrap gathered enough context."}"#.to_string(),
        r#"{"verdict":"ok","reason":"Stopping after bootstrap is correct for this test."}"#
            .to_string(),
    ]));
    let runner = runner_with(driver, provider, dir.path(), AgentConfig::default()).await;

    let result = runner
        .run(AgentRunRequest {
            task: "Gather context and stop.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should succeed");

    let transcript = load_persisted_session(&FileSessionStore::new(dir.path()), &result.session_id)
        .await
        .expect("persisted transcript should load")
        .expect("persisted session should exist");
    let replayed_again =
        load_persisted_session(&FileSessionStore::new(dir.path()), &result.session_id)
            .await
            .expect("replayed transcript should load again")
            .expect("persisted session should still exist");

    assert_eq!(transcript, replayed_again);
    assert_eq!(transcript.session.id, result.session_id);
    assert_eq!(transcript.session.status, SessionStatus::Running);
    assert_eq!(transcript.events.len(), 5);
    assert!(matches!(
        &transcript.events[0],
        ReplayableTranscriptEvent::UserInput { text }
            if text == "Gather context and stop."
    ));
    assert!(matches!(
        &transcript.events[1],
        ReplayableTranscriptEvent::ToolCall { name, .. }
            if name == "capabilities"
    ));
    assert!(matches!(
        &transcript.events[2],
        ReplayableTranscriptEvent::ToolResult { result }
            if result.tool_name == "capabilities" && !result.is_error && result.read_only
    ));
    assert!(matches!(
        &transcript.events[3],
        ReplayableTranscriptEvent::ModelResponse { content }
            if content.contains("\"decision\":\"finish\"")
    ));
    assert_eq!(
        transcript.events[4],
        ReplayableTranscriptEvent::Completed {
            summary: Some("Bootstrap gathered enough context.".into()),
        }
    );
}

#[tokio::test]
async fn file_session_store_persists_terminal_error_events_without_agent_internal_state() {
    let dir = tempdir().unwrap();
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Err(OperatorError::Tool {
        tool: "click".into(),
        message: "button is disabled".into(),
    }));
    driver.push_action_result(Err(OperatorError::Tool {
        tool: "click".into(),
        message: "button is disabled".into(),
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"click","arguments":{},"summary":"Try clicking Save."}"#
            .to_string(),
        r#"{"decision":"call_tool","name":"click","arguments":{},"summary":"Try clicking Save again."}"#
            .to_string(),
    ]));
    let runner = runner_with(
        driver,
        provider,
        dir.path(),
        AgentConfig {
            repeated_error_limit: 1,
            ..AgentConfig::default()
        },
    )
    .await;

    let error = runner
        .run(AgentRunRequest {
            task: "Click Save.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect_err("runner should stop after repeated tool failures");

    match error {
        AgentError::Planner(message) => {
            assert!(
                message.contains("tool failure loop detected"),
                "unexpected planner stop reason: {message}"
            );
        }
        other => panic!("unexpected error kind: {other}"),
    }

    let session_store = FileSessionStore::new(dir.path());
    let session_id = session_store
        .list(Some(1))
        .await
        .expect("listing sessions should succeed")
        .into_iter()
        .next()
        .expect("failed run should create a persisted session");
    let transcript = load_persisted_session(&session_store, &session_id)
        .await
        .expect("error transcript should load")
        .expect("persisted session should exist");

    assert!(
        transcript.events.iter().all(|event| {
            let json = serde_json::to_value(event).expect("transcript events should serialize");
            json.get("parse_attempts").is_none()
                && json.get("notes").is_none()
                && json.get("latest_snapshot").is_none()
                && json.get("ui_state_stale").is_none()
        }),
        "runtime session transcript should not leak agent-only state: {transcript:?}"
    );
    assert!(matches!(
        transcript.events.last(),
        Some(ReplayableTranscriptEvent::Error { message })
            if message.contains("tool failure loop detected")
    ));
}
