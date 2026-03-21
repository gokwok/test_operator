use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use operator_core::{
    Action, ActionOutcome, ActionRequest, AppInfo, Capability, CapabilitySet, ExecContext, Locator,
    MouseButton, ObserveRequest, ObserveResult, OperatorError, PermissionStatus, PermissionsReport,
    QueryRequest, QueryResult, Surface, SurfaceKind, WindowInfo,
};
use operator_runtime::{
    AuditEvent, AuditEventKind, EventSink, RuntimeBuilder, RuntimeConfig, SnapshotStore,
};
use operator_testkit::{test_snapshot, InMemorySnapshotStore, MockPlatformDriver};
use serde_json::json;

#[tokio::test]
async fn snapshot_get_reads_from_store_without_driver() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let snapshot = test_snapshot("snap-1");
    store.save(&snapshot).await.unwrap();

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .build()
        .await
        .unwrap();

    let output = runtime
        .tools()
        .invoke("snapshot-get", json!({ "snapshot_id": "snap-1" }))
        .await
        .unwrap();

    assert_eq!(output["snapshot"]["id"], json!("snap-1"));
    assert_eq!(output["snapshot"]["metadata"]["platform"], json!("macos"));
}

#[tokio::test]
async fn observe_tool_extracts_exec_context_from_json() {
    let config = RuntimeConfig {
        default_timeout_ms: 250,
        ..RuntimeConfig::default()
    };

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    driver.push_observe_result(Ok(ObserveResult {
        snapshot: test_snapshot("snap-obs"),
    }));

    let runtime = RuntimeBuilder::new(config)
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let output = runtime
        .tools()
        .invoke(
            "observe",
            json!({
                "surface": { "kind": "Frontmost" },
                "include_screenshot": true,
                "include_elements": true
            }),
        )
        .await
        .unwrap();

    assert_eq!(output["snapshot"]["id"], json!("snap-obs"));

    let calls = driver.observe_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        (
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: true,
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
    );
}

#[tokio::test]
async fn read_only_query_tools_forward_runtime_results() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::AppLifecycle,
            Capability::WindowManagement,
            Capability::Permissions,
            Capability::Capture,
        ]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    }])));
    driver.push_query_result(Ok(QueryResult::Windows(vec![WindowInfo {
        id: 7.into(),
        title: Some("Calculator".into()),
        app_name: Some("Calculator".into()),
        bounds: None,
        is_focused: true,
        is_minimized: false,
    }])));
    driver.push_query_result(Ok(QueryResult::Permissions(PermissionsReport {
        screen_recording: PermissionStatus::Granted,
        accessibility: PermissionStatus::Denied,
    })));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke("list-apps", json!({ "target": "local:macos" }))
        .await
        .unwrap();
    let windows = runtime
        .tools()
        .invoke(
            "list-windows",
            json!({ "target": "local:macos", "app": "Calculator" }),
        )
        .await
        .unwrap();
    let permissions = runtime
        .tools()
        .invoke("permissions-status", json!({ "target": "local:macos" }))
        .await
        .unwrap();
    let capabilities = runtime
        .tools()
        .invoke("capabilities", json!({ "target": "local:macos" }))
        .await
        .unwrap();

    assert_eq!(apps["apps"][0]["name"], json!("Calculator"));
    assert_eq!(windows["windows"][0]["id"], json!(7));
    assert_eq!(permissions["permissions"]["accessibility"], json!("Denied"));
    assert_eq!(
        capabilities["capabilities"],
        json!(["AppLifecycle", "Capture", "Permissions", "WindowManagement"])
    );

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![
            (
                QueryRequest::ListApps,
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                QueryRequest::ListWindows {
                    app: Some("Calculator".into()),
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                QueryRequest::PermissionsStatus,
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
        ]
    );
}

#[tokio::test]
async fn action_tools_are_blocked_when_side_effects_are_disabled() {
    let events = Arc::new(RecordingEventSink::default());
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));

    let runtime = RuntimeBuilder::new(RuntimeConfig {
        allow_side_effects: false,
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .event_sink(events.clone())
    .register_driver(driver.clone())
    .build()
    .await
    .unwrap();

    let error = runtime
        .tools()
        .invoke(
            "click",
            json!({
                "target": "local:macos",
                "button": "Left"
            }),
        )
        .await
        .unwrap_err();

    match error {
        OperatorError::Tool { tool, message } => {
            assert_eq!(tool, "click");
            assert_eq!(message, "side effects are disabled by runtime policy");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(driver.action_calls().await.is_empty());
    assert!(matches!(
        events.events().as_slice(),
        [AuditEvent {
            kind: AuditEventKind::SideEffectBlocked { tool },
            ..
        }] if tool == "click"
    ));
}

#[tokio::test]
async fn action_tools_forward_typed_requests_to_runtime_act() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::PointerInput,
            Capability::KeyboardInput,
            Capability::AppLifecycle,
        ]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 12,
        detail: Some("clicked".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 18,
        detail: Some("typed".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("launched".into()),
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let click = runtime
        .tools()
        .invoke(
            "click",
            json!({
                "target": "local:macos",
                "button": "Right",
                "locator": {
                    "Text": "Submit"
                }
            }),
        )
        .await
        .unwrap();
    let typed = runtime
        .tools()
        .invoke(
            "type",
            json!({
                "target": "local:macos",
                "text": "hello world",
                "locator": {
                    "Text": "Search"
                }
            }),
        )
        .await
        .unwrap();
    let launched = runtime
        .tools()
        .invoke(
            "launch-app",
            json!({
                "target": "local:macos",
                "bundle_id_or_name": "Calculator"
            }),
        )
        .await
        .unwrap();

    assert_eq!(click["outcome"]["detail"], json!("clicked"));
    assert_eq!(typed["outcome"]["detail"], json!("typed"));
    assert_eq!(launched["outcome"]["detail"], json!("launched"));

    let calls = driver.action_calls().await;
    assert_eq!(
        calls,
        vec![
            (
                ActionRequest {
                    action: Action::Click {
                        button: MouseButton::Right,
                    },
                    locator: Some(Locator::Text("Submit".into())),
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Type {
                        text: "hello world".into(),
                    },
                    locator: Some(Locator::Text("Search".into())),
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::LaunchApp {
                        bundle_id_or_name: "Calculator".into(),
                    },
                    locator: None,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
        ]
    );
}

#[tokio::test]
async fn action_tools_export_stable_specs() {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .build()
        .await
        .unwrap();

    let specs = runtime.tools().specs();
    let names = specs.iter().map(|spec| spec.name).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "capabilities",
            "click",
            "launch-app",
            "list-apps",
            "list-windows",
            "observe",
            "permissions-status",
            "snapshot-get",
            "type",
        ]
    );

    for spec in &specs {
        assert_eq!(spec.input_schema["type"], json!("object"));
        assert_eq!(spec.output_schema["type"], json!("object"));
    }

    let click = specs.iter().find(|spec| spec.name == "click").unwrap();
    assert!(click.has_side_effects);
    assert_eq!(click.capabilities_required, &[Capability::PointerInput]);

    let launch_app = specs.iter().find(|spec| spec.name == "launch-app").unwrap();
    assert!(launch_app.has_side_effects);
    assert_eq!(
        launch_app.capabilities_required,
        &[Capability::AppLifecycle]
    );

    let type_spec = specs.iter().find(|spec| spec.name == "type").unwrap();
    assert!(type_spec.has_side_effects);
    assert_eq!(
        type_spec.capabilities_required,
        &[Capability::KeyboardInput]
    );

    let observe = specs.iter().find(|spec| spec.name == "observe").unwrap();
    assert!(!observe.has_side_effects);
    assert!(observe.input_schema["properties"]["surface"].is_object());

    let snapshot_get = specs
        .iter()
        .find(|spec| spec.name == "snapshot-get")
        .unwrap();
    assert!(!snapshot_get.has_side_effects);
    assert!(snapshot_get.input_schema["properties"]["snapshot_id"].is_object());
    assert_eq!(snapshot_get.capabilities_required.len(), 0);
}

#[derive(Default)]
struct RecordingEventSink {
    events: Mutex<Vec<AuditEvent>>,
}

impl RecordingEventSink {
    fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl EventSink for RecordingEventSink {
    async fn emit(&self, event: AuditEvent) -> Result<(), OperatorError> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}
