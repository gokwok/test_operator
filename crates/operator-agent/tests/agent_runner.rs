mod support;

use std::sync::Arc;

use operator_agent::{
    model::{Context, Message, ModelRegistry, ProviderKind, UserMessage},
    AgentConfig, AgentError, AgentRunRequest, AgentRunner,
};
use operator_core::{ArtifactId, Capability, CapabilitySet, OperatorError, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig, SessionEvent, SessionStore};
use operator_testkit::{
    test_snapshot, InMemorySessionStore, InMemorySnapshotStore, MockPlatformDriver,
};

use support::DeterministicTestProvider;

async fn runner_with(
    driver: Arc<MockPlatformDriver>,
    provider: Arc<DeterministicTestProvider>,
    session_store: Arc<InMemorySessionStore>,
    config: AgentConfig,
) -> AgentRunner {
    runner_with_provider_kind(
        driver,
        ProviderKind::OpenAi,
        provider,
        session_store,
        config,
    )
    .await
}

async fn runner_with_provider_kind(
    driver: Arc<MockPlatformDriver>,
    provider_kind: ProviderKind,
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
    models.register_provider(provider_kind, provider);

    AgentRunner::new(Arc::new(runtime), models, config)
}

fn current_request_text(context: &Context) -> &str {
    let Some(Message::User(UserMessage { content, .. })) = context.messages.last() else {
        panic!("planner request should append a final user message");
    };
    content
        .iter()
        .find_map(|block| match block {
            operator_agent::model::ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .expect("planner request should contain a text block")
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
async fn runner_executes_tool_then_finishes_without_finish_gate_reflection() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    let mut initial_snapshot = test_snapshot("snap-initial");
    initial_snapshot.root_ids.clear();
    initial_snapshot.elements.clear();
    initial_snapshot.image_artifact = Some(ArtifactId("capture-initial.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: initial_snapshot,
    }));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: test_snapshot("snap-runner"),
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"observe","arguments":{"surface":{"kind":"Frontmost"},"include_elements":true,"include_screenshot":false},"summary":"Capture the current UI."}"#.to_string(),
        r#"{"decision":"finish","summary":"Observed the frontmost UI."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver.clone(),
        provider.clone(),
        session_store.clone(),
        AgentConfig {
            include_elements: true,
            ..AgentConfig::default()
        },
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Observe the frontmost UI.".into(),
            target: TargetId("macos".into()),
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
    assert_eq!(driver.observe_calls().await.len(), 2);

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        2,
        "verified read-only completion should stay on the planner hot path"
    );

    let first_planner_request = current_request_text(&requests[0].context);
    assert!(first_planner_request.contains("Task\nObserve the frontmost UI."));
    assert!(first_planner_request.contains("- snapshot: snap-initial"));
    assert!(first_planner_request.contains("- stale: yes"));

    let second_planner_request = current_request_text(&requests[1].context);
    let tool_result_message = requests[1]
        .context
        .messages
        .iter()
        .find_map(|message| match message {
            Message::ToolResult(tool_result) => Some(tool_result),
            _ => None,
        })
        .expect("second planner request should include a model-context tool summary");
    let compact_tool_summary = tool_result_message
        .content
        .iter()
        .find_map(|block| match block {
            operator_agent::model::ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .expect("tool result summary should contain text");
    assert_eq!(
        compact_tool_summary,
        "snapshot snap-initial on frontmost (roots=0, elements=0), screenshot=capture-initial.png"
    );
    assert!(
        !compact_tool_summary.contains("\"snapshot\""),
        "planner-visible tool summaries should not inline persisted JSON: {compact_tool_summary}"
    );
    assert!(second_planner_request.contains("- snapshot: snap-runner"));
    assert!(
        second_planner_request.contains("observe [read-only]"),
        "second planner request should carry the observe result: {second_planner_request}"
    );
    assert!(second_planner_request.contains("- stale: no"));
}

#[tokio::test]
async fn runner_replans_when_finish_gate_rejects_false_finish() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::Capture,
            Capability::InspectTree,
            Capability::PointerInput,
        ]),
    ));
    let mut initial_snapshot = test_snapshot("snap-initial");
    initial_snapshot.root_ids.clear();
    initial_snapshot.elements.clear();
    initial_snapshot.image_artifact = Some(ArtifactId("capture-initial.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: initial_snapshot,
    }));
    driver.push_action_result(Ok(operator_core::ActionOutcome {
        success: true,
        duration_ms: 12,
        detail: Some("clicked Save".into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }));
    let mut after_click = test_snapshot("snap-after-click");
    after_click.root_ids.clear();
    after_click.elements.clear();
    after_click.image_artifact = Some(ArtifactId("capture-after-click.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: after_click,
    }));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: test_snapshot("snap-confirmed"),
    }));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"click","arguments":{},"summary":"Click Save before finishing."}"#
            .to_string(),
        r#"{"decision":"finish","summary":"The task is done."}"#.to_string(),
        r#"{"decision":"call_tool","name":"observe","arguments":{"surface":{"kind":"Frontmost"},"include_elements":true,"include_screenshot":false},"summary":"Verify the UI after clicking Save."}"#.to_string(),
        r#"{"decision":"finish","summary":"The task is now confirmed."}"#.to_string(),
        r#"{"verdict":"ok","reason":"The post-action observe confirms the UI after clicking Save."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver,
        provider.clone(),
        session_store,
        AgentConfig {
            include_elements: true,
            ..AgentConfig::default()
        },
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Click Save and confirm the UI after the click.".into(),
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should succeed after finish-gate feedback");

    assert_eq!(result.summary, "The task is now confirmed.");

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        5,
        "planner, early false finish, replan, final finish, finish gate"
    );

    let replanned_request = current_request_text(&requests[2].context);
    assert!(replanned_request.contains("Notes"));
    assert!(replanned_request.contains(
        "The task is not verified yet because there is no fresh usable observe result after the last UI change."
    ));
}

#[tokio::test]
async fn runner_avoids_structured_output_for_doubao_planner_and_finish_gate() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    let mut initial_snapshot = test_snapshot("snap-initial");
    initial_snapshot.root_ids.clear();
    initial_snapshot.elements.clear();
    initial_snapshot.image_artifact = Some(ArtifactId("capture-initial.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: initial_snapshot,
    }));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: test_snapshot("snap-confirmed"),
    }));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        "I will return only the JSON payload you need.\n```json\n{\"decision\":\"finish\",\"summary\":\"The task is confirmed.\"}\n```"
            .to_string(),
        "I will verify first.\n```json\n{\"decision\":\"call_tool\",\"name\":\"observe\",\"arguments\":{\"surface\":{\"kind\":\"Frontmost\"},\"include_elements\":true,\"include_screenshot\":false},\"summary\":\"Verify the current UI before finishing.\"}\n```"
            .to_string(),
        "I will return JSON only next time.\n```json\n{\"decision\":\"finish\",\"summary\":\"The task is confirmed.\"}\n```"
            .to_string(),
        "Finish gate complete.\n{\"verdict\":\"ok\",\"reason\":\"The wrapped JSON verdict is still recoverable without structured output.\"}"
            .to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with_provider_kind(
        driver,
        ProviderKind::OpenAiCompatible,
        provider.clone(),
        session_store,
        AgentConfig {
            include_elements: true,
            ..AgentConfig::default()
        },
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Finish from wrapped JSON without structured output after verification.".into(),
            target: TargetId("macos".into()),
            model: Some("doubao-seed".into()),
        })
        .await
        .expect("runner should parse wrapped planner and finish-gate JSON");

    assert_eq!(result.summary, "The task is confirmed.");

    let requests = provider.requests();
    assert_eq!(requests.len(), 4, "planner, replan, planner, finish gate");
    assert!(
        requests
            .iter()
            .all(|request| request.options.response_format.is_none()),
        "planner and finish gate should not request structured output: {requests:?}"
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
            target: TargetId("macos".into()),
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
        vec!["click".to_string(), "click".to_string(),]
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
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    let mut initial_snapshot = test_snapshot("snap-initial");
    initial_snapshot.root_ids.clear();
    initial_snapshot.elements.clear();
    initial_snapshot.image_artifact = Some(ArtifactId("capture-initial.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: initial_snapshot,
    }));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: test_snapshot("snap-verified"),
    }));
    let provider = Arc::new(DeterministicTestProvider::from_texts([
        "not valid json".to_string(),
        r#"{"decision":"call_tool","name":"observe","arguments":{"surface":{"kind":"Frontmost"},"include_elements":true,"include_screenshot":false},"summary":"Verify the UI before finishing."}"#.to_string(),
        r#"{"decision":"finish","summary":"Recovered after retry."}"#.to_string(),
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
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
        })
        .await
        .expect("runner should recover within the retry budget");

    assert_eq!(result.summary, "Recovered after retry.");

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        3,
        "planner retry, planner success, planner finish"
    );

    let retry_request = current_request_text(&requests[1].context);
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
    assert!(retry_request.contains("Task\nRetry after invalid JSON."));
}
