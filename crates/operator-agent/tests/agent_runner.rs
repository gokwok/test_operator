mod support;

use std::sync::Arc;

use operator_agent::{
    model::{Context, Message, ModelRegistry, ProviderKind, UserMessage},
    AgentConfig, AgentError, AgentRunRequest, AgentRunner,
};
use operator_core::{Capability, CapabilitySet, FocusInfo, OperatorError, QueryResult, TargetId};
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
    config: AgentConfig,
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

    AgentRunner::new(Arc::new(runtime), models, config)
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

#[tokio::test]
async fn runner_executes_tool_then_finishes_with_reflection() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    driver.push_query_result(Ok(QueryResult::Focus(Some(FocusInfo {
        role: "AXWindow".into(),
        label: Some("Desktop".into()),
        bounds: None,
        app_name: Some("Finder".into()),
    }))));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: test_snapshot("snap-runner"),
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"observe","arguments":{"surface":{"kind":"Frontmost"},"include_elements":true,"include_screenshot":false},"summary":"Capture the current UI."}"#.to_string(),
        r#"{"decision":"finish","summary":"Observed the frontmost UI."}"#.to_string(),
        r#"{"verdict":"ok","reason":"The observe result confirms the task outcome."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver.clone(),
        provider.clone(),
        session_store.clone(),
        AgentConfig::default(),
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Observe the frontmost UI.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should succeed");

    assert_eq!(result.summary, "Observed the frontmost UI.");

    let events = session_store
        .events(&result.session_id)
        .await
        .expect("session events should be readable");
    assert!(
        tool_call_names(&events).contains(&"observe".to_string()),
        "observe call should be persisted: {events:?}"
    );
    assert_eq!(driver.observe_calls().await.len(), 1);

    let requests = provider.requests();
    assert_eq!(requests.len(), 3, "two planner calls plus one reflector");

    let second_planner_request = current_request_json(&requests[1].context);
    assert_eq!(
        second_planner_request["latest_snapshot"]["id"],
        Value::String("snap-runner".into())
    );
    let tool_names = second_planner_request["recent_tool_results"]
        .as_array()
        .expect("recent tool results should be an array")
        .iter()
        .filter_map(|item| item.get("tool_name"))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        tool_names.contains(&Value::String("observe".into())),
        "second planner request should carry the observe result: {second_planner_request}"
    );
    assert_eq!(second_planner_request["ui_state_stale"], Value::Bool(false));
}

#[tokio::test]
async fn runner_replans_when_reflector_rejects_finish() {
    let driver = Arc::new(MockPlatformDriver::new("macos", CapabilitySet::new([])));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"finish","summary":"The task is done."}"#.to_string(),
        r#"{"verdict":"not_ok","reason":"Need a concrete confirmation before finishing."}"#
            .to_string(),
        r#"{"decision":"finish","summary":"The task is now confirmed."}"#.to_string(),
        r#"{"verdict":"ok","reason":"The second finish summary addresses the missing proof."}"#
            .to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver,
        provider.clone(),
        session_store,
        AgentConfig::default(),
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Confirm the task with a second pass.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should succeed after reflector feedback");

    assert_eq!(result.summary, "The task is now confirmed.");

    let requests = provider.requests();
    assert_eq!(requests.len(), 4, "planner, reflector, planner, reflector");

    let replanned_request = current_request_json(&requests[2].context);
    assert_eq!(
        replanned_request["notes"],
        Value::Array(vec![Value::String(
            "Need a concrete confirmation before finishing.".into()
        )])
    );
}

#[tokio::test]
async fn runner_stops_on_repeated_tool_failures() {
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
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver,
        provider,
        session_store.clone(),
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
            "capabilities".to_string(),
            "click".to_string(),
            "click".to_string(),
        ]
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            SessionEvent::Error { message } if message.contains("tool failure loop detected")
        )),
        "runner should persist the terminal error event: {events:?}"
    );
}

#[tokio::test]
async fn runner_retries_invalid_planner_output_before_continuing() {
    let driver = Arc::new(MockPlatformDriver::new("macos", CapabilitySet::new([])));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        "not valid json".to_string(),
        r#"{"decision":"finish","summary":"Recovered after retry."}"#.to_string(),
        r#"{"verdict":"ok","reason":"The retry produced a valid finish decision."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver,
        provider.clone(),
        session_store.clone(),
        AgentConfig {
            max_parse_attempts: 2,
            ..AgentConfig::default()
        },
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Retry after invalid JSON.".into(),
            target: TargetId("local:macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should recover within the retry budget");

    assert_eq!(result.summary, "Recovered after retry.");

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        3,
        "planner retry, planner success, reflector"
    );

    let retry_request = current_request_json(&requests[1].context);
    let last_feedback = requests[1]
        .context
        .messages
        .iter()
        .rev()
        .nth(1)
        .expect("retry prompt should include the planner feedback message");
    let Message::User(UserMessage { content, .. }) = last_feedback else {
        panic!("feedback should be encoded as a user message");
    };
    let feedback_text = content
        .iter()
        .find_map(|block| match block {
            operator_agent::model::ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .expect("feedback message should contain a text block");
    assert!(
        feedback_text.contains("planner.feedback.v1"),
        "retry prompt should include planner feedback: {feedback_text}"
    );
    assert_eq!(
        retry_request["task"],
        Value::String("Retry after invalid JSON.".into())
    );
}
