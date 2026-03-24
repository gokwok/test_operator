mod support;

use std::sync::Arc;

use operator_agent::{
    model::{ModelRegistry, ProviderKind},
    AgentConfig, AgentRunRequest, AgentRunner,
};
use operator_core::{
    Capability, CapabilitySet, FocusInfo, PermissionStatus, PermissionsReport, QueryResult,
    TargetId,
};
use operator_runtime::{RuntimeBuilder, RuntimeConfig, SessionEvent, SessionStore};
use operator_testkit::{InMemorySessionStore, InMemorySnapshotStore, MockPlatformDriver};

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

fn tool_call_names(events: &[SessionEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::ToolCall { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn bootstrap_creates_runtime_session_and_records_optional_context_queries() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Permissions, Capability::InspectTree]),
    ));
    driver.push_query_result(Ok(QueryResult::Permissions(PermissionsReport {
        screen_recording: PermissionStatus::Granted,
        accessibility: PermissionStatus::Denied,
    })));
    driver.push_query_result(Ok(QueryResult::Focus(Some(FocusInfo {
        role: "AXWindow".into(),
        label: Some("Finder".into()),
        bounds: None,
        app_name: Some("Finder".into()),
    }))));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"finish","summary":"Bootstrap gathered enough context."}"#.to_string(),
        r#"{"verdict":"ok","reason":"The task intentionally ends after bootstrap."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(driver.clone(), provider.clone(), session_store.clone()).await;

    let result = runner
        .run(AgentRunRequest {
            task: "Gather context and stop.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should finish cleanly");

    let session = session_store
        .get(&result.session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session metadata should exist");
    assert_eq!(session.task, "Gather context and stop.");

    let events = session_store
        .events(&result.session_id)
        .await
        .expect("session events should be readable");
    assert!(
        matches!(&events[0], SessionEvent::UserInput { text } if text == "Gather context and stop."),
        "first event should persist the task input: {events:?}"
    );
    assert_eq!(
        tool_call_names(&events),
        vec![
            "capabilities".to_string(),
            "permissions-status".to_string(),
            "get-focus".to_string(),
        ]
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            SessionEvent::ModelResponse { content }
                if content.contains("\"decision\":\"finish\"")
        )),
        "planner response should be persisted: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            SessionEvent::Completed { summary: Some(summary) }
                if summary == "Bootstrap gathered enough context."
        )),
        "completed summary should be persisted: {events:?}"
    );

    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "planner + reflector should be called");
    assert_eq!(driver.query_calls().await.len(), 2);
}

#[tokio::test]
async fn bootstrap_skips_optional_queries_when_the_catalog_does_not_expose_them() {
    let driver = Arc::new(MockPlatformDriver::new("macos", CapabilitySet::new([])));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"finish","summary":"Nothing else is available."}"#.to_string(),
        r#"{"verdict":"ok","reason":"Stopping immediately is correct for this test."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(driver.clone(), provider, session_store.clone()).await;

    let result = runner
        .run(AgentRunRequest {
            task: "Do the minimum.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should finish cleanly");

    let events = session_store
        .events(&result.session_id)
        .await
        .expect("session events should be readable");
    assert_eq!(tool_call_names(&events), vec!["capabilities".to_string()]);
    assert!(
        driver.query_calls().await.is_empty(),
        "bootstrap should not hit optional query tools when they are unavailable"
    );
}
