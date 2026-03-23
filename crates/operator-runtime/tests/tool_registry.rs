use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use operator_core::{
    Action, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionTargetSelector, AppInfo,
    ArtifactId, Capability, CapabilitySet, ClickMode, DragModifier, DragMotion, ExecContext,
    FocusInfo, Locator, ObserveRequest, ObserveResult, OperatorError, PermissionStatus,
    PermissionsReport, QueryRequest, QueryResult, Rect, Surface, SurfaceKind, TypeTrailingKey,
    WindowInfo,
};
use operator_runtime::{
    AuditEvent, AuditEventKind, EventSink, FileArtifactStore, RuntimeBuilder, RuntimeConfig,
    SnapshotStore,
};
use operator_testkit::{test_snapshot, InMemorySnapshotStore, MockPlatformDriver};
use serde_json::json;
use tempfile::tempdir;

fn default_action_request() -> ActionRequest {
    ActionRequest {
        action: Action::Move,
        locator: None,
        target_selector: None,
        focus_policy: ActionFocusPolicy::Auto,
    }
}

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
async fn artifact_get_reads_from_artifact_store_without_driver() {
    let dir = tempdir().unwrap();
    let store = Arc::new(FileArtifactStore::new(dir.path()));
    let artifact_id = ArtifactId("capture-1.png".into());
    let artifact_path = store.artifacts_dir().join(&artifact_id.0);
    std::fs::create_dir_all(store.artifacts_dir()).unwrap();
    std::fs::write(&artifact_path, b"png-bytes").unwrap();

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .artifact_store(store)
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .build()
        .await
        .unwrap();

    let output = runtime
        .tools()
        .invoke("artifact-get", json!({ "artifact_id": "capture-1.png" }))
        .await
        .unwrap();

    assert_eq!(output["artifact"]["id"], json!("capture-1.png"));
    assert_eq!(
        output["artifact"]["path"],
        json!(artifact_path.to_string_lossy().to_string())
    );
}

#[tokio::test]
async fn artifact_get_returns_not_found_when_artifact_is_missing() {
    let dir = tempdir().unwrap();
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .artifact_store(Arc::new(FileArtifactStore::new(dir.path())))
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .build()
        .await
        .unwrap();

    let error = runtime
        .tools()
        .invoke("artifact-get", json!({ "artifact_id": "missing.png" }))
        .await
        .unwrap_err();

    match error {
        OperatorError::Tool { tool, message } => {
            assert_eq!(tool, "artifact-get");
            assert_eq!(message, "artifact not found: missing.png");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn artifact_get_rejects_invalid_artifact_ids() {
    let dir = tempdir().unwrap();
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .artifact_store(Arc::new(FileArtifactStore::new(dir.path())))
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .build()
        .await
        .unwrap();

    for invalid_id in ["../escape.png", "nested\\escape.png"] {
        let error = runtime
            .tools()
            .invoke("artifact-get", json!({ "artifact_id": invalid_id }))
            .await
            .unwrap_err();

        match error {
            OperatorError::Tool { tool, message } => {
                assert_eq!(tool, "artifact-get");
                assert!(message.contains("invalid artifact id"));
            }
            other => panic!("unexpected error for {invalid_id}: {other:?}"),
        }
    }
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
async fn get_focus_query_tool_forwards_runtime_results() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::InspectTree]),
    ));
    driver.push_query_result(Ok(QueryResult::Focus(Some(FocusInfo {
        role: "AXTextField".into(),
        label: Some("Search".into()),
        bounds: Some(Rect {
            x: 40.0,
            y: 60.0,
            width: 280.0,
            height: 32.0,
        }),
        app_name: Some("Safari".into()),
    }))));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let focus = runtime
        .tools()
        .invoke("get-focus", json!({ "target": "local:macos" }))
        .await
        .unwrap();

    assert_eq!(focus["focus"]["role"], json!("AXTextField"));
    assert_eq!(focus["focus"]["app_name"], json!("Safari"));

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![(
            QueryRequest::GetFocus,
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
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
                "mode": "Left"
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
            Capability::WindowManagement,
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
        duration_ms: 14,
        detail: Some("scrolled".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 6,
        detail: Some("moved".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 16,
        detail: Some("dragged".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 15,
        detail: Some("swiped".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 11,
        detail: Some("sent hotkey".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 7,
        detail: Some("pressed down 3 times".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("launched".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("closed window 41".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("minimized window 42".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("maximized window 43".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 10,
        detail: Some("switched app".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 8,
        detail: Some("quit app".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 13,
        detail: Some("relaunched app".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 6,
        detail: Some("hid app".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 6,
        detail: Some("unhid app".into()),
    }));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 5,
        detail: Some("focused".into()),
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
                "mode": "Double",
                "target_selector": {
                    "WindowTitle": "Submit Sheet"
                },
                "focus_policy": "Never",
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
                "clear_before": true,
                "delay_ms": 25,
                "trailing_keys": ["Return", "Tab"],
                "target_selector": {
                    "App": "TextEdit"
                },
                "focus_policy": "Auto",
                "locator": {
                    "Text": "Search"
                }
            }),
        )
        .await
        .unwrap();
    let scrolled = runtime
        .tools()
        .invoke(
            "scroll",
            json!({
                "target": "local:macos",
                "locator": {
                    "Text": "Results"
                },
                "delta_x": 0.0,
                "delta_y": -120.0
            }),
        )
        .await
        .unwrap();
    let moved = runtime
        .tools()
        .invoke(
            "move",
            json!({
                "target": "local:macos",
                "locator": {
                    "Coords": {
                        "x": 320.0,
                        "y": 240.0
                    }
                }
            }),
        )
        .await
        .unwrap();
    let dragged = runtime
        .tools()
        .invoke(
            "drag",
            json!({
                "target": "local:macos",
                "from": {
                    "Coords": {
                        "x": 10.0,
                        "y": 20.0
                    }
                },
                "to": {
                    "Coords": {
                        "x": 30.0,
                        "y": 60.0
                    }
                }
            }),
        )
        .await
        .unwrap();
    let swiped = runtime
        .tools()
        .invoke(
            "swipe",
            json!({
                "target": "local:macos",
                "from": {
                    "Coords": {
                        "x": 15.0,
                        "y": 25.0
                    }
                },
                "to": {
                    "Coords": {
                        "x": 90.0,
                        "y": 25.0
                    }
                },
                "duration_ms": 240,
                "steps": 4
            }),
        )
        .await
        .unwrap();
    let hotkey = runtime
        .tools()
        .invoke(
            "hotkey",
            json!({
                "target": "local:macos",
                "keys": ["command", "shift", "p"]
            }),
        )
        .await
        .unwrap();
    let pressed = runtime
        .tools()
        .invoke(
            "press",
            json!({
                "target": "local:macos",
                "key": "down",
                "count": 3
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
    let closed = runtime
        .tools()
        .invoke(
            "close-window",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "WindowTitle": "Draft"
                },
                "focus_policy": "Never"
            }),
        )
        .await
        .unwrap();
    let minimized = runtime
        .tools()
        .invoke(
            "minimize-window",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "App": "TextEdit"
                },
                "focus_policy": "Auto"
            }),
        )
        .await
        .unwrap();
    let maximized = runtime
        .tools()
        .invoke(
            "maximize-window",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "Pid": 101
                },
                "focus_policy": "Auto"
            }),
        )
        .await
        .unwrap();
    let switched = runtime
        .tools()
        .invoke(
            "switch-app",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "App": "TextEdit"
                }
            }),
        )
        .await
        .unwrap();
    let quit = runtime
        .tools()
        .invoke(
            "quit-app",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "Pid": 101
                }
            }),
        )
        .await
        .unwrap();
    let relaunched = runtime
        .tools()
        .invoke(
            "relaunch-app",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "WindowTitle": "Draft"
                }
            }),
        )
        .await
        .unwrap();
    let hid = runtime
        .tools()
        .invoke(
            "hide-app",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "WindowIndex": 2
                }
            }),
        )
        .await
        .unwrap();
    let unhid = runtime
        .tools()
        .invoke(
            "unhide-app",
            json!({
                "target": "local:macos",
                "target_selector": {
                    "WindowId": 77
                }
            }),
        )
        .await
        .unwrap();
    let focused = runtime
        .tools()
        .invoke(
            "focus-window",
            json!({
                "target": "local:macos",
                "window_id": 42
            }),
        )
        .await
        .unwrap();

    assert_eq!(click["outcome"]["detail"], json!("clicked"));
    assert_eq!(typed["outcome"]["detail"], json!("typed"));
    assert_eq!(scrolled["outcome"]["detail"], json!("scrolled"));
    assert_eq!(moved["outcome"]["detail"], json!("moved"));
    assert_eq!(dragged["outcome"]["detail"], json!("dragged"));
    assert_eq!(swiped["outcome"]["detail"], json!("swiped"));
    assert_eq!(hotkey["outcome"]["detail"], json!("sent hotkey"));
    assert_eq!(pressed["outcome"]["detail"], json!("pressed down 3 times"));
    assert_eq!(launched["outcome"]["detail"], json!("launched"));
    assert_eq!(closed["outcome"]["detail"], json!("closed window 41"));
    assert_eq!(minimized["outcome"]["detail"], json!("minimized window 42"));
    assert_eq!(maximized["outcome"]["detail"], json!("maximized window 43"));
    assert_eq!(switched["outcome"]["detail"], json!("switched app"));
    assert_eq!(quit["outcome"]["detail"], json!("quit app"));
    assert_eq!(relaunched["outcome"]["detail"], json!("relaunched app"));
    assert_eq!(hid["outcome"]["detail"], json!("hid app"));
    assert_eq!(unhid["outcome"]["detail"], json!("unhid app"));
    assert_eq!(focused["outcome"]["detail"], json!("focused"));

    let calls = driver.action_calls().await;
    assert_eq!(
        calls,
        vec![
            (
                ActionRequest {
                    action: Action::Click {
                        mode: ClickMode::Double
                    },
                    locator: Some(Locator::Text("Submit".into())),
                    target_selector: Some(ActionTargetSelector::WindowTitle("Submit Sheet".into())),
                    focus_policy: ActionFocusPolicy::Never,
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
                        clear_before: true,
                        delay_ms: Some(25),
                        trailing_keys: vec![TypeTrailingKey::Return, TypeTrailingKey::Tab],
                    },
                    locator: Some(Locator::Text("Search".into())),
                    target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Scroll {
                        delta_x: 0.0,
                        delta_y: -120.0,
                    },
                    locator: Some(Locator::Text("Results".into())),
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Move,
                    locator: Some(Locator::Coords(operator_core::Point { x: 320.0, y: 240.0 })),
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Drag {
                        from: Locator::Coords(operator_core::Point { x: 10.0, y: 20.0 }),
                        to: Locator::Coords(operator_core::Point { x: 30.0, y: 60.0 }),
                        motion: DragMotion::default(),
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Swipe {
                        from: Locator::Coords(operator_core::Point { x: 15.0, y: 25.0 }),
                        to: Locator::Coords(operator_core::Point { x: 90.0, y: 25.0 }),
                        duration_ms: Some(240),
                        steps: Some(4.try_into().unwrap()),
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Hotkey {
                        keys: vec!["command".into(), "shift".into(), "p".into()],
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::Press {
                        key: "down".into(),
                        count: 3.try_into().unwrap(),
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
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
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::CloseWindow,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                    focus_policy: ActionFocusPolicy::Never,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::MinimizeWindow,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::MaximizeWindow,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::Pid(101)),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::SwitchApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::QuitApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::Pid(101)),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::RelaunchApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::HideApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::WindowIndex(2)),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::UnhideApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::WindowId(77.into())),
                    focus_policy: ActionFocusPolicy::Auto,
                },
                ExecContext {
                    target: "local:macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::FocusWindow { id: 42.into() },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
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
            "artifact-get",
            "capabilities",
            "click",
            "close-window",
            "drag",
            "focus-window",
            "get-focus",
            "hide-app",
            "hotkey",
            "launch-app",
            "list-apps",
            "list-windows",
            "maximize-window",
            "minimize-window",
            "move",
            "observe",
            "permissions-status",
            "press",
            "quit-app",
            "relaunch-app",
            "scroll",
            "snapshot-get",
            "swipe",
            "switch-app",
            "type",
            "unhide-app",
        ]
    );

    for spec in &specs {
        assert_eq!(spec.input_schema["type"], json!("object"));
        assert_eq!(spec.output_schema["type"], json!("object"));
    }

    let click = specs.iter().find(|spec| spec.name == "click").unwrap();
    assert!(click.has_side_effects);
    assert_eq!(click.capabilities_required, &[Capability::PointerInput]);
    assert!(click.input_schema["properties"]["mode"].is_object());
    assert!(click.input_schema["properties"]["button"].is_null());
    assert!(click.input_schema["properties"]["target_selector"].is_object());
    assert!(click.input_schema["properties"]["focus_policy"].is_object());

    let drag = specs.iter().find(|spec| spec.name == "drag").unwrap();
    assert!(drag.has_side_effects);
    assert_eq!(drag.capabilities_required, &[Capability::PointerInput]);
    assert!(drag.input_schema["properties"]["duration_ms"].is_object());
    assert!(drag.input_schema["properties"]["steps"].is_object());
    assert!(drag.input_schema["properties"]["modifiers"].is_object());

    let launch_app = specs.iter().find(|spec| spec.name == "launch-app").unwrap();
    assert!(launch_app.has_side_effects);
    assert_eq!(
        launch_app.capabilities_required,
        &[Capability::AppLifecycle]
    );

    for tool_name in ["close-window", "maximize-window", "minimize-window"] {
        let spec = specs.iter().find(|spec| spec.name == tool_name).unwrap();
        assert!(spec.has_side_effects);
        assert_eq!(spec.capabilities_required, &[Capability::WindowManagement]);
        assert!(spec.input_schema["properties"]["target_selector"].is_object());
        assert!(spec.input_schema["properties"]["focus_policy"].is_object());
    }

    for tool_name in [
        "switch-app",
        "quit-app",
        "relaunch-app",
        "hide-app",
        "unhide-app",
    ] {
        let spec = specs.iter().find(|spec| spec.name == tool_name).unwrap();
        assert!(spec.has_side_effects);
        assert_eq!(spec.capabilities_required, &[Capability::AppLifecycle]);
        assert!(spec.input_schema["properties"]["target_selector"].is_object());
        assert!(spec.input_schema["properties"]["focus_policy"].is_null());
    }

    let scroll = specs.iter().find(|spec| spec.name == "scroll").unwrap();
    assert!(scroll.has_side_effects);
    assert_eq!(scroll.capabilities_required, &[Capability::PointerInput]);
    assert!(scroll.input_schema["properties"]["locator"].is_object());

    let move_spec = specs.iter().find(|spec| spec.name == "move").unwrap();
    assert!(move_spec.has_side_effects);
    assert_eq!(move_spec.capabilities_required, &[Capability::PointerInput]);
    assert!(move_spec.input_schema["properties"]["locator"].is_object());
    assert!(move_spec.input_schema["properties"]["target_selector"].is_object());
    assert!(move_spec.input_schema["properties"]["focus_policy"].is_object());

    let focus_window = specs
        .iter()
        .find(|spec| spec.name == "focus-window")
        .unwrap();
    assert!(focus_window.has_side_effects);
    assert_eq!(
        focus_window.capabilities_required,
        &[Capability::WindowManagement]
    );

    let hotkey = specs.iter().find(|spec| spec.name == "hotkey").unwrap();
    assert!(hotkey.has_side_effects);
    assert_eq!(hotkey.capabilities_required, &[Capability::KeyboardInput]);

    let press = specs.iter().find(|spec| spec.name == "press").unwrap();
    assert!(press.has_side_effects);
    assert_eq!(press.capabilities_required, &[Capability::KeyboardInput]);
    assert!(press.input_schema["properties"]["key"].is_object());
    assert!(press.input_schema["properties"]["count"].is_object());
    assert!(press.input_schema["properties"]["target_selector"].is_object());
    assert!(press.input_schema["properties"]["focus_policy"].is_object());

    let swipe = specs.iter().find(|spec| spec.name == "swipe").unwrap();
    assert!(swipe.has_side_effects);
    assert_eq!(swipe.capabilities_required, &[Capability::PointerInput]);
    assert!(swipe.input_schema["properties"]["duration_ms"].is_object());
    assert!(swipe.input_schema["properties"]["steps"].is_object());
    assert!(swipe.input_schema["properties"]["modifiers"].is_null());

    let type_spec = specs.iter().find(|spec| spec.name == "type").unwrap();
    assert!(type_spec.has_side_effects);
    assert_eq!(
        type_spec.capabilities_required,
        &[Capability::KeyboardInput]
    );
    assert!(type_spec.input_schema["properties"]["clear_before"].is_object());
    assert!(type_spec.input_schema["properties"]["delay_ms"].is_object());
    assert!(type_spec.input_schema["properties"]["trailing_keys"].is_object());
    assert!(type_spec.input_schema["properties"]["target_selector"].is_object());
    assert!(type_spec.input_schema["properties"]["focus_policy"].is_object());

    let observe = specs.iter().find(|spec| spec.name == "observe").unwrap();
    assert!(!observe.has_side_effects);
    assert!(observe.input_schema["properties"]["surface"].is_object());

    let get_focus = specs.iter().find(|spec| spec.name == "get-focus").unwrap();
    assert!(!get_focus.has_side_effects);
    assert_eq!(get_focus.capabilities_required, &[Capability::InspectTree]);

    let snapshot_get = specs
        .iter()
        .find(|spec| spec.name == "snapshot-get")
        .unwrap();
    assert!(!snapshot_get.has_side_effects);
    assert!(snapshot_get.input_schema["properties"]["snapshot_id"].is_object());
    assert_eq!(snapshot_get.capabilities_required.len(), 0);

    let artifact_get = specs
        .iter()
        .find(|spec| spec.name == "artifact-get")
        .unwrap();
    assert!(!artifact_get.has_side_effects);
    assert!(artifact_get.input_schema["properties"]["artifact_id"].is_object());
    assert_eq!(artifact_get.capabilities_required.len(), 0);
}

#[tokio::test]
async fn drag_tool_forwards_motion_options_to_runtime_act() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 16,
        detail: Some("dragged".into()),
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let dragged = runtime
        .tools()
        .invoke(
            "drag",
            json!({
                "target": "local:macos",
                "from": {
                    "Coords": {
                        "x": 10.0,
                        "y": 20.0
                    }
                },
                "to": {
                    "Coords": {
                        "x": 30.0,
                        "y": 60.0
                    }
                },
                "duration_ms": 300,
                "steps": 6,
                "modifiers": ["Command", "Shift"]
            }),
        )
        .await
        .unwrap();

    assert_eq!(dragged["outcome"]["detail"], json!("dragged"));
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::Drag {
                    from: Locator::Coords(operator_core::Point { x: 10.0, y: 20.0 }),
                    to: Locator::Coords(operator_core::Point { x: 30.0, y: 60.0 }),
                    motion: DragMotion {
                        duration_ms: Some(300),
                        steps: Some(6.try_into().unwrap()),
                        modifiers: vec![DragModifier::Command, DragModifier::Shift],
                    },
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test]
async fn swipe_tool_forwards_motion_options_to_runtime_act() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 15,
        detail: Some("swiped".into()),
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let swiped = runtime
        .tools()
        .invoke(
            "swipe",
            json!({
                "target": "local:macos",
                "from": {
                    "Coords": {
                        "x": 12.0,
                        "y": 24.0
                    }
                },
                "to": {
                    "Coords": {
                        "x": 240.0,
                        "y": 24.0
                    }
                },
                "duration_ms": 180,
                "steps": 3
            }),
        )
        .await
        .unwrap();

    assert_eq!(swiped["outcome"]["detail"], json!("swiped"));
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::Swipe {
                    from: Locator::Coords(operator_core::Point { x: 12.0, y: 24.0 }),
                    to: Locator::Coords(operator_core::Point { x: 240.0, y: 24.0 }),
                    duration_ms: Some(180),
                    steps: Some(3.try_into().unwrap()),
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
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
