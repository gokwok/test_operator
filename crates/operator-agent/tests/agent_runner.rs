mod support;

use std::sync::{Arc, Mutex};

use operator_agent::{
    model::{Context, Message, ModelProvider, ModelRegistry, ProviderKind, UserMessage},
    AgentConfig, AgentError, AgentProgressEvent, AgentProgressReporter, AgentRunRequest,
    AgentRunner,
};
use operator_core::{
    Action, ActionRequest, ArtifactId, Capability, CapabilitySet, OperatorError, TargetId,
};
use operator_runtime::{RuntimeBuilder, RuntimeConfig, SessionEvent, SessionStore};
use operator_testkit::{
    test_snapshot, InMemorySessionStore, InMemorySnapshotStore, MockPlatformDriver,
};
use tokio::sync::Notify;

use support::{model_provider::BlockingTestProvider, DeterministicTestProvider};

async fn runner_with(
    driver: Arc<MockPlatformDriver>,
    provider: Arc<dyn ModelProvider>,
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
    provider: Arc<dyn ModelProvider>,
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

fn planner_tool_summary_text(context: &Context, name: &str) -> String {
    let tool = context
        .tools
        .iter()
        .find(|tool| tool.name.as_ref() == name)
        .expect("planner request should include the expected tool");
    serde_json::to_string(&tool.input_schema).expect("tool summary should serialize")
}

#[derive(Default)]
struct RecordingProgressReporter {
    events: Mutex<Vec<AgentProgressEvent>>,
}

impl RecordingProgressReporter {
    fn events(&self) -> Vec<AgentProgressEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl AgentProgressReporter for RecordingProgressReporter {
    fn report(&self, event: AgentProgressEvent) {
        self.events.lock().unwrap().push(event);
    }
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
            app: None,
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
async fn runner_reports_concise_progress_events_for_turns_tools_and_completion() {
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
        snapshot: test_snapshot("snap-progress"),
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"observe","arguments":{"surface":{"kind":"Frontmost"},"include_elements":true,"include_screenshot":false},"summary":"Capture the current UI."}"#.to_string(),
        r#"{"decision":"finish","summary":"The current UI is captured."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let progress = Arc::new(RecordingProgressReporter::default());
    let runner = runner_with(
        driver,
        provider,
        session_store,
        AgentConfig {
            include_elements: true,
            ..AgentConfig::default()
        },
    )
    .await
    .with_progress_reporter(progress.clone());

    let result = runner
        .run(AgentRunRequest {
            task: "Capture the frontmost UI.".into(),
            target: TargetId("macos".into()),
            model: Some("openai".into()),
            app: None,
        })
        .await
        .expect("runner should succeed");

    assert_eq!(result.summary, "The current UI is captured.");

    let events = progress.events();
    assert_eq!(
        events[0],
        AgentProgressEvent::RunStarted {
            session_id: result.session_id.clone(),
            target: TargetId("macos".into()),
            model: "openai".into(),
            task: "Capture the frontmost UI.".into(),
        }
    );
    assert!(events.iter().any(|e| matches!(e,
        AgentProgressEvent::ToolCall { turn_index: 0, step_index: 0, name, .. }
        if name == "observe"
    )));
    assert!(events.contains(&AgentProgressEvent::TurnStarted { turn_index: 1 }));
    assert!(events.contains(&AgentProgressEvent::PlannedTool {
        turn_index: 1,
        tool_name: "observe".into(),
        summary: "Capture the current UI.".into(),
    }));
    assert!(events.contains(&AgentProgressEvent::ToolResult {
        turn_index: 1,
        step_index: 1,
        name: "observe".into(),
        summary: "snapshot snap-progress on frontmost (roots=1, elements=1)".into(),
        is_error: false,
    }));
    assert_eq!(
        events.last(),
        Some(&AgentProgressEvent::RunCompleted {
            summary: "The current UI is captured.".into(),
        })
    );
}

#[tokio::test]
async fn runner_marks_interrupted_runs_in_session_store() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let provider = Arc::new(BlockingTestProvider::new(started.clone(), release.clone()));
    let session_store = Arc::new(InMemorySessionStore::new());
    let interrupt = Arc::new(Notify::new());
    let runner = runner_with(
        driver,
        provider,
        session_store.clone(),
        AgentConfig::default(),
    )
    .await
    .with_interrupt_notify(interrupt.clone());

    let handle = tokio::spawn(async move {
        runner
            .run(AgentRunRequest {
                task: "Interrupt this run.".into(),
                target: TargetId("macos".into()),
                model: Some("gpt-5.4".into()),
                app: None,
            })
            .await
    });

    started.notified().await;
    interrupt.notify_one();

    let error = handle
        .await
        .expect("task should join")
        .expect_err("run should interrupt");
    match error {
        AgentError::Interrupted(message) => {
            assert!(
                message.contains("ctrl-c"),
                "unexpected interrupt message: {message}"
            );
        }
        other => panic!("unexpected error kind: {other}"),
    }
    release.notify_waiters();

    let session_id = session_store
        .list(Some(1))
        .await
        .expect("listing sessions should succeed")
        .into_iter()
        .next()
        .expect("interrupted run should persist a session");
    let session = session_store
        .get(&session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");
    assert_eq!(session.status, operator_runtime::SessionStatus::Interrupted);

    let events = session_store
        .events(&session_id)
        .await
        .expect("session events should be readable");
    assert!(matches!(
        events.last(),
        Some(SessionEvent::Error { message }) if message.contains("ctrl-c")
    ));
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
            app: None,
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
async fn runner_normalizes_frontmost_direct_type_before_execution() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::KeyboardInput]),
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
        duration_ms: 8,
        detail: Some("typed expression".into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }));
    let mut after_type = test_snapshot("snap-after-type");
    after_type.root_ids.clear();
    after_type.elements.clear();
    after_type.image_artifact = Some(ArtifactId("capture-after-type.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: after_type,
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"call_tool","name":"type","arguments":{"text":"777*999=","target_selector":{"App":"Calculator"},"verifications":["Focus","WindowState"]},"summary":"Type the expression into Calculator."}"#
            .to_string(),
        r#"{"decision":"finish","summary":"Typed the expression into Calculator."}"#.to_string(),
        r#"{"verdict":"ok","reason":"The refreshed frontmost screenshot confirms the expression was typed."}"#
            .to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver.clone(),
        provider,
        session_store,
        AgentConfig::default(),
    )
    .await;

    let result = runner
        .run(AgentRunRequest {
            task: "Type 777*999= into the frontmost Calculator window.".into(),
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
            app: None,
        })
        .await
        .expect("runner should normalize the direct frontmost type action");

    assert_eq!(result.summary, "Typed the expression into Calculator.");

    let action_calls = driver.action_calls().await;
    assert_eq!(action_calls.len(), 1);
    assert_eq!(
        action_calls[0].0,
        ActionRequest {
            action: Action::Type {
                text: "777*999=".into(),
                clear_before: false,
                delay_ms: None,
                trailing_keys: Vec::new(),
            },
            locator: None,
            target_selector: None,
            focus_policy: Default::default(),
            verifications: Vec::new(),
        }
    );
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
            app: None,
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
            app: None,
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
            app: None,
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

#[tokio::test]
async fn runner_hides_selector_locators_without_element_backed_observation() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::PointerInput]),
    ));
    let mut screenshot_only = test_snapshot("snap-harmony");
    screenshot_only.root_ids.clear();
    screenshot_only.elements.clear();
    screenshot_only.image_artifact = Some(ArtifactId("capture-harmony.png".into()));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: screenshot_only,
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"finish","summary":"The current UI is already visible."}"#.to_string(),
    ]));
    let session_store = Arc::new(InMemorySessionStore::new());
    let runner = runner_with(
        driver,
        provider.clone(),
        session_store,
        AgentConfig::default(),
    )
    .await;

    runner
        .run(AgentRunRequest {
            task: "Inspect the current UI.".into(),
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
            app: None,
        })
        .await
        .expect("runner should succeed");

    let requests = provider.requests();
    let click_summary = planner_tool_summary_text(&requests[0].context, "click");
    assert!(
        !click_summary.contains("SnapshotElement")
            && !click_summary.contains("\"Text\"")
            && !click_summary.contains("\"Role\""),
        "selector locators should stay hidden without an element-backed observation: {click_summary}"
    );
}

#[tokio::test]
async fn agent_runner_reenables_selector_locators_after_element_backed_observation() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::Capture,
            Capability::InspectTree,
            Capability::PointerInput,
        ]),
    ));
    driver.push_observe_result(Ok(operator_core::ObserveResult {
        snapshot: test_snapshot("snap-elements"),
    }));

    let provider = Arc::new(DeterministicTestProvider::from_texts([
        r#"{"decision":"finish","summary":"The current UI is ready."}"#.to_string(),
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

    runner
        .run(AgentRunRequest {
            task: "Inspect the current desktop UI.".into(),
            target: TargetId("macos".into()),
            model: Some("gpt-5.4".into()),
            app: None,
        })
        .await
        .expect("runner should succeed");

    let requests = provider.requests();
    let click_summary = planner_tool_summary_text(&requests[0].context, "click");
    assert!(
        click_summary.contains("SnapshotElement")
            || click_summary.contains("\"Text\"")
            || click_summary.contains("\"Role\""),
        "selector locators should return once the current observation includes elements: {click_summary}"
    );
    let request = current_request_text(&requests[0].context);
    assert!(request
        .contains("element digest (SnapshotElement ids; native bounds use device coordinates):"));
    assert!(request.contains("[el-1] AXButton label=\"Test Element el-1\" enabled=true"));
}
