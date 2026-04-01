mod support;

use std::{sync::Arc, time::SystemTime};

use operator_agent::{
    model::{ModelError, ModelRegistry, ProviderKind},
    session::SessionJournal,
    AgentConfig, AgentError, AgentRunRequest, AgentRunner,
};
use operator_core::{ArtifactId, Capability, CapabilitySet, SessionId, TargetId};
use operator_runtime::{
    RuntimeBuilder, RuntimeConfig, Session, SessionEvent, SessionStatus, SessionStore,
};
use operator_testkit::{
    test_snapshot, InMemorySessionStore, InMemorySnapshotStore, MockPlatformDriver,
};

use support::DeterministicTestProvider;

async fn runner_with(
    driver: Arc<MockPlatformDriver>,
    provider: Arc<DeterministicTestProvider>,
    session_store: Arc<InMemorySessionStore>,
) -> AgentRunner {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .session_store(session_store)
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");

    let mut models = ModelRegistry::new();
    models.register_provider(ProviderKind::OpenAi, provider);

    AgentRunner::new(Arc::new(runtime), models, AgentConfig::default())
}

#[tokio::test]
async fn session_journal_buffers_events_until_flush() {
    let store = InMemorySessionStore::new();
    let session_id = SessionId("sess-journal".into());
    store
        .create(&Session {
            id: session_id.clone(),
            created_at: SystemTime::UNIX_EPOCH,
            task: "Inspect buffering.".into(),
            status: SessionStatus::Running,
        })
        .await
        .expect("session should be created");

    let mut journal = SessionJournal::new(session_id.clone());
    journal.record(SessionEvent::UserInput {
        text: "Inspect buffering.".into(),
    });
    journal.record(SessionEvent::ModelResponse {
        content: "{\"decision\":\"finish\"}".into(),
    });

    assert_eq!(journal.pending_len(), 2);
    assert!(
        store
            .events(&session_id)
            .await
            .expect("session events should be readable")
            .is_empty(),
        "journaled events should stay in memory until flush"
    );

    journal
        .flush(&store)
        .await
        .expect("journal flush should persist pending events");

    assert!(journal.is_empty());
    assert_eq!(
        store
            .events(&session_id)
            .await
            .expect("flushed session events should be readable"),
        vec![
            SessionEvent::UserInput {
                text: "Inspect buffering.".into(),
            },
            SessionEvent::ModelResponse {
                content: "{\"decision\":\"finish\"}".into(),
            },
        ]
    );
}

#[tokio::test]
async fn session_journal_flushes_buffered_events_on_fail_fast_exit() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture]),
    ));
    let mut initial_snapshot = test_snapshot("snap-initial");
    initial_snapshot.root_ids.clear();
    initial_snapshot.elements.clear();
    initial_snapshot.image_artifact = Some(ArtifactId("capture-initial.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: initial_snapshot,
    }));

    let provider = Arc::new(DeterministicTestProvider::from_results([Err(
        ModelError::Protocol("planner transport dropped mid-turn".into()),
    )]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(driver, provider, session_store.clone()).await;

    let error = runner
        .run(AgentRunRequest {
            task: "Prime the planner context and fail fast.".into(),
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
            app: None,
        })
        .await
        .expect_err("runner should bubble the planner failure");

    match error {
        AgentError::Planner(message) => {
            assert!(
                message.contains("planner model call failed"),
                "unexpected fail-fast planner error: {message}"
            );
        }
        other => panic!("unexpected error kind: {other}"),
    }

    let session_id = session_store
        .list(Some(1))
        .await
        .expect("listing sessions should succeed")
        .into_iter()
        .next()
        .expect("the fail-fast run should still create a session");

    let events = session_store
        .events(&session_id)
        .await
        .expect("session events should be readable after fail-fast flush");
    assert_eq!(
        events.len(),
        3,
        "fail-fast flush should keep buffered events"
    );
    assert!(matches!(
        &events[0],
        SessionEvent::UserInput { text }
            if text == "Prime the planner context and fail fast."
    ));
    assert!(matches!(
        &events[1],
        SessionEvent::ToolCall { name, input }
            if name == "observe"
                && input["include_screenshot"] == true
                && input["include_elements"] == false
    ));

    let SessionEvent::ToolResult { name, output } = &events[2] else {
        panic!("expected the buffered observe result to flush on fail-fast exit: {events:?}");
    };
    assert_eq!(name, "observe");
    assert_eq!(output["tool_name"], "observe");
    assert_eq!(output["arguments"]["include_screenshot"], true);
    assert_eq!(output["arguments"]["include_elements"], false);
    assert_eq!(output["is_error"], false);
    assert_eq!(output["read_only"], true);
    assert_eq!(output["output"]["snapshot"]["id"], "snap-initial");
    assert_eq!(
        output["output"]["snapshot"]["image_artifact"],
        "capture-initial.png"
    );
}
