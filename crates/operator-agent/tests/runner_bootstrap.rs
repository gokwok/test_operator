mod support;

use std::sync::Arc;

use operator_agent::{
    model::{Context, Message, ModelRegistry, ProviderKind, UserMessage},
    AgentConfig, AgentError, AgentRunRequest, AgentRunner,
};
use operator_core::{ActionOutcome, ArtifactId, Capability, CapabilitySet, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig, SessionEvent, SessionStore};
use operator_testkit::{
    test_snapshot, InMemorySessionStore, InMemorySnapshotStore, MockPlatformDriver,
};
use serde_json::Value;

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

fn current_request_json(context: &Context) -> Value {
    let Some(Message::User(UserMessage { content, .. })) = context.messages.last() else {
        panic!("planner request should append a final user message");
    };
    let text = content
        .iter()
        .find_map(|block| match block {
            operator_agent::model::ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .expect("planner request should contain a text block");

    serde_json::from_str(text).expect("planner request payload should be valid json")
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

fn screenshot_only_snapshot(snapshot_id: &str, artifact_id: &str) -> operator_core::ObserveResult {
    let mut snapshot = test_snapshot(snapshot_id);
    snapshot.root_ids.clear();
    snapshot.elements.clear();
    snapshot.image_artifact = Some(ArtifactId(artifact_id.into()));
    operator_core::ObserveResult { snapshot }
}

#[tokio::test]
async fn auto_observe_primes_the_first_planner_turn_without_bootstrap_queries() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture]),
    ));
    driver.push_observe_result(Ok(screenshot_only_snapshot(
        "snap-initial",
        "capture-initial.png",
    )));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"fail","reason":"Stop after inspecting the first planner context."}"#
            .to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(driver.clone(), provider.clone(), session_store.clone()).await;

    let error = runner
        .run(AgentRunRequest {
            task: "Inspect the first planner context and stop.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect_err("runner should stop after the planned fail decision");

    match error {
        AgentError::Planner(message) => {
            assert_eq!(message, "Stop after inspecting the first planner context.");
        }
        other => panic!("unexpected error kind: {other}"),
    }

    let session_id = session_store
        .list(Some(1))
        .await
        .expect("listing sessions should succeed")
        .into_iter()
        .next()
        .expect("the failed run should still create a session");

    let session = session_store
        .get(&session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session metadata should exist");
    assert_eq!(session.task, "Inspect the first planner context and stop.");

    let events = session_store
        .events(&session_id)
        .await
        .expect("session events should be readable");
    assert!(
        matches!(
            &events[0],
            SessionEvent::UserInput { text } if text == "Inspect the first planner context and stop."
        ),
        "first event should persist the task input: {events:?}"
    );
    assert_eq!(tool_call_names(&events), vec!["observe".to_string()]);
    assert!(
        events.iter().any(|event| matches!(
            event,
            SessionEvent::ModelResponse { content } if content.contains("\"decision\":\"fail\"")
        )),
        "planner response should be persisted: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            SessionEvent::Error { message }
                if message == "Stop after inspecting the first planner context."
        )),
        "terminal fail reason should be persisted: {events:?}"
    );

    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "only the planner should be called");

    let first_request = current_request_json(&requests[0].context);
    assert_eq!(
        first_request["latest_snapshot"]["id"],
        Value::String("snap-initial".into())
    );
    assert_eq!(first_request["ui_state_stale"], Value::Bool(true));
    assert_eq!(
        first_request["recent_tool_results"][0]["tool_name"],
        Value::String("observe".into())
    );

    assert!(driver.query_calls().await.is_empty());
    let observe_calls = driver.observe_calls().await;
    assert_eq!(observe_calls.len(), 1);
    assert!(observe_calls[0].0.include_screenshot);
    assert!(!observe_calls[0].0.include_elements);
}

#[tokio::test]
async fn auto_observe_refreshes_after_successful_side_effect_tools() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::PointerInput]),
    ));
    driver.push_observe_result(Ok(screenshot_only_snapshot(
        "snap-initial",
        "capture-initial.png",
    )));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 12,
        detail: Some("clicked Save".into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }));
    driver.push_observe_result(Ok(screenshot_only_snapshot(
        "snap-after-click",
        "capture-after-click.png",
    )));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"click","arguments":{},"summary":"Click Save."}"#
            .to_string(),
        r#"{"decision":"fail","reason":"Stop after the auto observe refresh."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(driver.clone(), provider.clone(), session_store.clone()).await;

    let error = runner
        .run(AgentRunRequest {
            task: "Click Save and stop after the automatic refresh.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect_err("runner should stop after the planned fail decision");

    match error {
        AgentError::Planner(message) => {
            assert_eq!(message, "Stop after the auto observe refresh.");
        }
        other => panic!("unexpected error kind: {other}"),
    }

    let session_id = session_store
        .list(Some(1))
        .await
        .expect("listing sessions should succeed")
        .into_iter()
        .next()
        .expect("the failed run should still create a session");

    let events = session_store
        .events(&session_id)
        .await
        .expect("session events should be readable");
    assert_eq!(
        tool_call_names(&events),
        vec![
            "observe".to_string(),
            "click".to_string(),
            "observe".to_string(),
        ]
    );

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        2,
        "planner should see the refreshed post-action context"
    );
    let second_request = current_request_json(&requests[1].context);
    assert_eq!(
        second_request["latest_snapshot"]["id"],
        Value::String("snap-after-click".into())
    );

    assert!(driver.query_calls().await.is_empty());
    let observe_calls = driver.observe_calls().await;
    assert_eq!(observe_calls.len(), 2);
    assert!(observe_calls
        .iter()
        .all(|(req, _)| req.include_screenshot && !req.include_elements));
}
