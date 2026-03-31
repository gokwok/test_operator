use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use operator_core::{
    Action, ActionCoordinates, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionSideEffect,
    ActionTargetSelector, ActionVerification, AppInfo, AppListFilter, AppListMode, ArtifactId,
    Capability, CapabilitySet, ClickMode, DragModifier, DragMotion, ExecContext, FocusInfo,
    Locator, ObserveRequest, ObserveResult, OperatorError, PermissionCheck, PermissionStatus,
    PermissionsReport, Point, QueryRequest, QueryResult, Rect, Surface, SurfaceKind,
    TypeTrailingKey, WindowInfo,
};
use operator_runtime::{
    AuditEvent, AuditEventKind, EventSink, FileArtifactStore, NamedTargetConfig, RuntimeBuilder,
    RuntimeConfig, SnapshotStore,
};
use operator_testkit::{test_snapshot, InMemorySnapshotStore, MockPlatformDriver};
use serde_json::{json, Value};
use tempfile::tempdir;

fn default_action_request() -> ActionRequest {
    ActionRequest {
        action: Action::Move,
        locator: None,
        target_selector: None,
        focus_policy: ActionFocusPolicy::Auto,
        verifications: Vec::new(),
    }
}

fn successful_action_outcome(detail: &str, duration_ms: u64) -> ActionOutcome {
    ActionOutcome {
        success: true,
        duration_ms,
        detail: Some(detail.into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }
}

fn schema_ref<'a>(schema: &'a Value, reference: &str) -> &'a Value {
    let key = reference.rsplit('/').next().unwrap();
    schema
        .get("$defs")
        .and_then(|defs| defs.get(key))
        .or_else(|| schema.get("definitions").and_then(|defs| defs.get(key)))
        .unwrap_or_else(|| panic!("missing schema reference: {reference}"))
}

fn verification_enum_values(schema: &Value) -> Vec<String> {
    let verifications = &schema["properties"]["verifications"];
    if verifications.is_null() {
        return Vec::new();
    }

    let items = &verifications["items"];
    let enum_schema = if let Some(reference) = items["$ref"].as_str() {
        schema_ref(schema, reference)
    } else {
        items
    };

    enum_schema["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
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
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
    );
}

#[tokio::test]
async fn observe_tool_allows_capture_only_driver_when_elements_are_disabled() {
    let mut config = RuntimeConfig {
        default_timeout_ms: 250,
        default_target: "harmony-pc".into(),
        ..RuntimeConfig::default()
    };
    config.targets.insert(
        "harmony-pc".into(),
        NamedTargetConfig {
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            description: None,
            driver_config: Default::default(),
        },
    );

    let mut snapshot = test_snapshot("snap-harmony");
    snapshot.target = "harmony-pc".into();
    snapshot.metadata.platform = "harmony".into();
    snapshot.elements.clear();
    snapshot.root_ids.clear();

    let driver = Arc::new(MockPlatformDriver::with_driver_id(
        "harmony",
        "harmony.hdc",
        CapabilitySet::new([Capability::Capture]),
    ));
    driver.push_observe_result(Ok(ObserveResult {
        snapshot: snapshot.clone(),
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
                "include_elements": false
            }),
        )
        .await
        .unwrap();

    assert_eq!(output["snapshot"]["id"], json!("snap-harmony"));

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
                include_elements: false,
            },
            ExecContext {
                target: "harmony-pc".into(),
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
            Capability::WindowQuery,
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
    driver.push_query_result(Ok(QueryResult::Permissions(PermissionsReport::new([
        PermissionCheck::new("accessibility", "Accessibility", PermissionStatus::Denied)
            .with_message("Accessibility permission is required for macOS automation."),
        PermissionCheck::new("system_events", "System Events", PermissionStatus::Granted)
            .with_message(
                "System Events access is required for macOS window queries and focus reads.",
            ),
        PermissionCheck::new(
            "screen_recording",
            "Screen Recording",
            PermissionStatus::Granted,
        )
        .with_message("Screen Recording permission is required for macOS capture."),
    ]))));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke("list-apps", json!({ "target": "macos" }))
        .await
        .unwrap();
    let windows = runtime
        .tools()
        .invoke(
            "list-windows",
            json!({ "target": "macos", "app": "Calculator" }),
        )
        .await
        .unwrap();
    let permissions = runtime
        .tools()
        .invoke("permissions-status", json!({ "target": "macos" }))
        .await
        .unwrap();
    let capabilities = runtime
        .tools()
        .invoke("capabilities", json!({ "target": "macos" }))
        .await
        .unwrap();

    assert_eq!(apps["apps"][0]["name"], json!("Calculator"));
    assert_eq!(windows["windows"][0]["id"], json!(7));
    assert_eq!(
        permissions["permissions"]["checks"][0]["id"],
        json!("accessibility")
    );
    assert_eq!(
        permissions["permissions"]["checks"][0]["status"],
        json!("Denied")
    );
    assert_eq!(
        permissions["permissions"]["checks"][1]["status"],
        json!("Granted")
    );
    assert_eq!(
        capabilities["capabilities"],
        json!(["AppLifecycle", "Capture", "Permissions", "WindowQuery"])
    );

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![
            (
                QueryRequest::ListApps {
                    mode: AppListMode::Running,
                    filter: AppListFilter::default(),
                },
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                QueryRequest::ListWindows {
                    app: Some("Calculator".into()),
                },
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                QueryRequest::PermissionsStatus,
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
        ]
    );
}

#[tokio::test]
async fn list_apps_tool_forwards_explicit_all_mode() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![
        AppInfo {
            bundle_id: Some("com.apple.Calculator".into()),
            name: "Calculator".into(),
            pid: None,
            is_running: false,
        },
        AppInfo {
            bundle_id: Some("com.apple.TextEdit".into()),
            name: "TextEdit".into(),
            pid: Some(101),
            is_running: true,
        },
    ])));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke("list-apps", json!({ "target": "macos", "mode": "all" }))
        .await
        .unwrap();

    assert_eq!(apps["apps"][0]["name"], json!("Calculator"));
    assert_eq!(apps["apps"][0]["is_running"], json!(false));
    assert_eq!(apps["apps"][1]["name"], json!("TextEdit"));
    assert_eq!(apps["apps"][1]["is_running"], json!(true));

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![(
            QueryRequest::ListApps {
                mode: AppListMode::All,
                filter: AppListFilter::default(),
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test]
async fn list_apps_tool_defaults_filtered_queries_to_all_mode() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![AppInfo {
        bundle_id: Some("com.openai.codex".into()),
        name: "Codex".into(),
        pid: Some(42),
        is_running: true,
    }])));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke(
            "list-apps",
            json!({
                "target": "macos",
                "name": "Cod",
                "bundle": "com.openai.codex"
            }),
        )
        .await
        .unwrap();

    assert_eq!(apps["apps"][0]["name"], json!("Codex"));

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![(
            QueryRequest::ListApps {
                mode: AppListMode::All,
                filter: AppListFilter {
                    name: Some("Cod".into()),
                    bundle: Some("com.openai.codex".into()),
                },
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test]
async fn read_only_query_tools_support_harmony_query_surface_without_inspect_tree() {
    let mut config = RuntimeConfig {
        default_timeout_ms: 250,
        default_target: "harmony-pc".into(),
        ..RuntimeConfig::default()
    };
    config.targets.insert(
        "harmony-pc".into(),
        NamedTargetConfig {
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            description: None,
            driver_config: Default::default(),
        },
    );

    let driver = Arc::new(MockPlatformDriver::with_driver_id(
        "harmony",
        "harmony.hdc",
        CapabilitySet::new([
            Capability::Capture,
            Capability::AppLifecycle,
            Capability::WindowQuery,
            Capability::Permissions,
        ]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![AppInfo {
        bundle_id: Some("com.demo.notes".into()),
        name: "com.demo.notes".into(),
        pid: None,
        is_running: true,
    }])));
    driver.push_query_result(Ok(QueryResult::Windows(vec![WindowInfo {
        id: 7.into(),
        title: Some("Draft.txt".into()),
        app_name: Some("Notes".into()),
        bounds: Some(Rect {
            x: 40.0,
            y: 50.0,
            width: 600.0,
            height: 400.0,
        }),
        is_focused: true,
        is_minimized: false,
    }])));
    driver.push_query_result(Ok(QueryResult::Permissions(PermissionsReport::new([
        PermissionCheck::new("hdc.connect", "HDC Connect", PermissionStatus::Granted),
        PermissionCheck::new("hdc.shell", "HDC Shell", PermissionStatus::Granted),
        PermissionCheck::new("hdc.capture", "HDC Capture", PermissionStatus::Granted),
        PermissionCheck::new("hdc.ui_bridge", "HDC UI Bridge", PermissionStatus::Denied)
            .with_message("ui bridge unavailable"),
    ]))));

    let runtime = RuntimeBuilder::new(config)
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke("list-apps", json!({}))
        .await
        .unwrap();
    let windows = runtime
        .tools()
        .invoke("list-windows", json!({ "app": "Notes" }))
        .await
        .unwrap();
    let permissions = runtime
        .tools()
        .invoke("permissions-status", json!({}))
        .await
        .unwrap();
    let capabilities = runtime
        .tools()
        .invoke("capabilities", json!({}))
        .await
        .unwrap();
    let focus_error = runtime
        .tools()
        .invoke("get-focus", json!({}))
        .await
        .unwrap_err();

    assert_eq!(apps["apps"][0]["bundle_id"], json!("com.demo.notes"));
    assert_eq!(windows["windows"][0]["app_name"], json!("Notes"));
    assert_eq!(
        permissions["permissions"]["checks"][3]["status"],
        json!("Denied")
    );
    assert_eq!(
        capabilities["capabilities"],
        json!(["AppLifecycle", "Capture", "Permissions", "WindowQuery"])
    );
    assert!(matches!(
        focus_error,
        OperatorError::CapabilityNotSupported(Capability::InspectTree)
    ));

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![
            (
                QueryRequest::ListApps {
                    mode: AppListMode::Running,
                    filter: AppListFilter::default(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                QueryRequest::ListWindows {
                    app: Some("Notes".into()),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                QueryRequest::PermissionsStatus,
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
        ]
    );
}

#[tokio::test]
async fn list_apps_tool_defaults_filtered_harmony_queries_to_all_mode() {
    let mut config = RuntimeConfig {
        default_timeout_ms: 250,
        default_target: "harmony-pc".into(),
        ..RuntimeConfig::default()
    };
    config.targets.insert(
        "harmony-pc".into(),
        NamedTargetConfig {
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            description: None,
            driver_config: Default::default(),
        },
    );

    let driver = Arc::new(MockPlatformDriver::with_driver_id(
        "harmony",
        "harmony.hdc",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![AppInfo {
        bundle_id: Some("com.huawei.hmos.notepad".into()),
        name: "com.huawei.hmos.notepad".into(),
        pid: None,
        is_running: false,
    }])));

    let runtime = RuntimeBuilder::new(config)
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke("list-apps", json!({ "name": "note" }))
        .await
        .unwrap();

    assert_eq!(
        apps["apps"][0]["bundle_id"],
        json!("com.huawei.hmos.notepad")
    );

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![(
            QueryRequest::ListApps {
                mode: AppListMode::All,
                filter: AppListFilter {
                    name: Some("note".into()),
                    bundle: None,
                },
            },
            ExecContext {
                target: "harmony-pc".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )]
    );
}

#[tokio::test]
async fn list_apps_tool_defaults_filtered_harmony_bundle_queries_to_all_mode() {
    let mut config = RuntimeConfig {
        default_timeout_ms: 250,
        default_target: "harmony-pc".into(),
        ..RuntimeConfig::default()
    };
    config.targets.insert(
        "harmony-pc".into(),
        NamedTargetConfig {
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            description: None,
            driver_config: Default::default(),
        },
    );

    let driver = Arc::new(MockPlatformDriver::with_driver_id(
        "harmony",
        "harmony.hdc",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![AppInfo {
        bundle_id: Some("com.huawei.hmos.notepad".into()),
        name: "备忘录".into(),
        pid: None,
        is_running: false,
    }])));

    let runtime = RuntimeBuilder::new(config)
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let apps = runtime
        .tools()
        .invoke("list-apps", json!({ "bundle": "com.huawei.hmos.notepad" }))
        .await
        .unwrap();

    assert_eq!(apps["apps"][0]["name"], json!("备忘录"));

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![(
            QueryRequest::ListApps {
                mode: AppListMode::All,
                filter: AppListFilter {
                    name: None,
                    bundle: Some("com.huawei.hmos.notepad".into()),
                },
            },
            ExecContext {
                target: "harmony-pc".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )]
    );
}

#[tokio::test]
async fn action_tools_support_harmony_pointer_and_keyboard_surface_without_inspect_tree() {
    let mut config = RuntimeConfig {
        default_timeout_ms: 250,
        default_target: "harmony-pc".into(),
        ..RuntimeConfig::default()
    };
    config.targets.insert(
        "harmony-pc".into(),
        NamedTargetConfig {
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            description: None,
            driver_config: Default::default(),
        },
    );

    let driver = Arc::new(MockPlatformDriver::with_driver_id(
        "harmony",
        "harmony.hdc",
        CapabilitySet::new([
            Capability::Capture,
            Capability::PointerInput,
            Capability::KeyboardInput,
            Capability::AppLifecycle,
            Capability::WindowQuery,
            Capability::Permissions,
        ]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("clicked", 12)));
    driver.push_action_result(Ok(successful_action_outcome("typed", 18)));
    driver.push_action_result(Ok(successful_action_outcome("pressed down 2 times", 7)));
    driver.push_action_result(Ok(successful_action_outcome("sent hotkey", 11)));
    driver.push_action_result(Ok(successful_action_outcome("dragged", 16)));
    driver.push_action_result(Ok(successful_action_outcome("swiped", 15)));

    let runtime = RuntimeBuilder::new(config)
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
                "locator": {
                    "Coords": {
                        "x": 320.0,
                        "y": 240.0
                    }
                },
                "mode": "Right"
            }),
        )
        .await
        .unwrap();
    let typed = runtime
        .tools()
        .invoke(
            "type",
            json!({
                "text": "hello harmony",
                "clear_before": true,
                "trailing_keys": ["Return", "Escape"],
                "locator": {
                    "Coords": {
                        "x": 410.0,
                        "y": 280.0
                    }
                }
            }),
        )
        .await
        .unwrap();
    let pressed = runtime
        .tools()
        .invoke(
            "press",
            json!({
                "key": "down",
                "count": 2
            }),
        )
        .await
        .unwrap();
    let hotkey = runtime
        .tools()
        .invoke(
            "hotkey",
            json!({
                "keys": ["command", "shift", "p"]
            }),
        )
        .await
        .unwrap();
    let dragged = runtime
        .tools()
        .invoke(
            "drag",
            json!({
                "from": {
                    "Coords": {
                        "x": 10.0,
                        "y": 20.0
                    }
                },
                "to": {
                    "Coords": {
                        "x": 110.0,
                        "y": 20.0
                    }
                },
                "duration_ms": 250,
                "steps": 6,
                "modifiers": ["Command", "Shift"]
            }),
        )
        .await
        .unwrap();
    let swiped = runtime
        .tools()
        .invoke(
            "swipe",
            json!({
                "from": {
                    "Coords": {
                        "x": 12.0,
                        "y": 24.0
                    }
                },
                "to": {
                    "Coords": {
                        "x": 212.0,
                        "y": 24.0
                    }
                },
                "duration_ms": 500,
                "steps": 3
            }),
        )
        .await
        .unwrap();

    assert_eq!(click["outcome"]["detail"], json!("clicked"));
    assert_eq!(typed["outcome"]["detail"], json!("typed"));
    assert_eq!(pressed["outcome"]["detail"], json!("pressed down 2 times"));
    assert_eq!(hotkey["outcome"]["detail"], json!("sent hotkey"));
    assert_eq!(dragged["outcome"]["detail"], json!("dragged"));
    assert_eq!(swiped["outcome"]["detail"], json!("swiped"));

    assert_eq!(
        driver.action_calls().await,
        vec![
            (
                ActionRequest {
                    action: Action::Click {
                        mode: ClickMode::Right
                    },
                    locator: Some(Locator::Coords(operator_core::Point { x: 320.0, y: 240.0 })),
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::Type {
                        text: "hello harmony".into(),
                        clear_before: true,
                        delay_ms: None,
                        trailing_keys: vec![TypeTrailingKey::Return, TypeTrailingKey::Escape],
                    },
                    locator: Some(Locator::Coords(operator_core::Point { x: 410.0, y: 280.0 })),
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::Press {
                        key: "down".into(),
                        count: 2.try_into().unwrap(),
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::Drag {
                        from: Locator::Coords(operator_core::Point { x: 10.0, y: 20.0 }),
                        to: Locator::Coords(operator_core::Point { x: 110.0, y: 20.0 }),
                        motion: DragMotion {
                            duration_ms: Some(250),
                            steps: Some(6.try_into().unwrap()),
                            modifiers: vec![DragModifier::Command, DragModifier::Shift],
                        },
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::Swipe {
                        from: Locator::Coords(operator_core::Point { x: 12.0, y: 24.0 }),
                        to: Locator::Coords(operator_core::Point { x: 212.0, y: 24.0 }),
                        duration_ms: Some(500),
                        steps: Some(3.try_into().unwrap()),
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
        ]
    );
}

#[tokio::test]
async fn action_tools_support_harmony_app_lifecycle_surface_without_inspect_tree() {
    let mut config = RuntimeConfig {
        default_timeout_ms: 250,
        default_target: "harmony-pc".into(),
        ..RuntimeConfig::default()
    };
    config.targets.insert(
        "harmony-pc".into(),
        NamedTargetConfig {
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            description: None,
            driver_config: Default::default(),
        },
    );

    let driver = Arc::new(MockPlatformDriver::with_driver_id(
        "harmony",
        "harmony.hdc",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("launched com.demo.notes", 12)));
    driver.push_action_result(Ok(successful_action_outcome("switched app", 10)));
    driver.push_action_result(Ok(successful_action_outcome("quit app", 8)));
    driver.push_action_result(Ok(successful_action_outcome("relaunched app", 13)));

    let runtime = RuntimeBuilder::new(config)
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let launched = runtime
        .tools()
        .invoke(
            "launch-app",
            json!({
                "bundle_id_or_name": "Notes"
            }),
        )
        .await
        .unwrap();
    let switched = runtime
        .tools()
        .invoke(
            "switch-app",
            json!({
                "target_selector": {
                    "App": "Notes"
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
                "target_selector": {
                    "WindowTitle": "Draft"
                }
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        launched["outcome"]["detail"],
        json!("launched com.demo.notes")
    );
    assert_eq!(switched["outcome"]["detail"], json!("switched app"));
    assert_eq!(quit["outcome"]["detail"], json!("quit app"));
    assert_eq!(relaunched["outcome"]["detail"], json!("relaunched app"));

    assert_eq!(
        driver.action_calls().await,
        vec![
            (
                ActionRequest {
                    action: Action::LaunchApp {
                        bundle_id_or_name: "Notes".into(),
                    },
                    locator: None,
                    target_selector: None,
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::SwitchApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::App("Notes".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::QuitApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::Pid(101)),
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
                },
            ),
            (
                ActionRequest {
                    action: Action::RelaunchApp,
                    locator: None,
                    target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "harmony-pc".into(),
                    session: None,
                    timeout_ms: Some(250),
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
        bundle_id: Some("com.apple.Safari".into()),
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
        .invoke("get-focus", json!({ "target": "macos" }))
        .await
        .unwrap();

    assert_eq!(focus["focus"]["role"], json!("AXTextField"));
    assert_eq!(focus["focus"]["bundle_id"], json!("com.apple.Safari"));
    assert_eq!(focus["focus"]["app_name"], json!("Safari"));

    let calls = driver.query_calls().await;
    assert_eq!(
        calls,
        vec![(
            QueryRequest::GetFocus,
            ExecContext {
                target: "macos".into(),
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
                "target": "macos",
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
    driver.push_action_result(Ok(successful_action_outcome("clicked", 12)));
    driver.push_action_result(Ok(successful_action_outcome("typed", 18)));
    driver.push_action_result(Ok(successful_action_outcome("scrolled", 14)));
    driver.push_action_result(Ok(successful_action_outcome("moved", 6)));
    driver.push_action_result(Ok(successful_action_outcome("dragged", 16)));
    driver.push_action_result(Ok(successful_action_outcome("swiped", 15)));
    driver.push_action_result(Ok(successful_action_outcome("sent hotkey", 11)));
    driver.push_action_result(Ok(successful_action_outcome("pressed down 3 times", 7)));
    driver.push_action_result(Ok(successful_action_outcome("launched", 9)));
    driver.push_action_result(Ok(successful_action_outcome("closed window 41", 9)));
    driver.push_action_result(Ok(successful_action_outcome("minimized window 42", 9)));
    driver.push_action_result(Ok(successful_action_outcome("maximized window 43", 9)));
    driver.push_action_result(Ok(successful_action_outcome(
        "moved window 42 to x=120 y=240 width=640 height=480",
        9,
    )));
    driver.push_action_result(Ok(successful_action_outcome(
        "resized window 42 to x=120 y=240 width=800 height=600",
        9,
    )));
    driver.push_action_result(Ok(successful_action_outcome(
        "set window 42 bounds to x=80 y=120 width=900 height=700",
        9,
    )));
    driver.push_action_result(Ok(successful_action_outcome("switched app", 10)));
    driver.push_action_result(Ok(successful_action_outcome("quit app", 8)));
    driver.push_action_result(Ok(successful_action_outcome("relaunched app", 13)));
    driver.push_action_result(Ok(successful_action_outcome("hid app", 6)));
    driver.push_action_result(Ok(successful_action_outcome("unhid app", 6)));
    driver.push_action_result(Ok(successful_action_outcome("focused", 5)));

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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
                "target_selector": {
                    "Pid": 101
                },
                "focus_policy": "Auto"
            }),
        )
        .await
        .unwrap();
    let moved_window = runtime
        .tools()
        .invoke(
            "move-window",
            json!({
                "target": "macos",
                "target_selector": {
                    "WindowId": 42
                },
                "focus_policy": "Never",
                "x": 120.0,
                "y": 240.0
            }),
        )
        .await
        .unwrap();
    let resized_window = runtime
        .tools()
        .invoke(
            "resize-window",
            json!({
                "target": "macos",
                "target_selector": {
                    "App": "TextEdit"
                },
                "focus_policy": "Auto",
                "width": 800.0,
                "height": 600.0
            }),
        )
        .await
        .unwrap();
    let set_window_bounds = runtime
        .tools()
        .invoke(
            "set-window-bounds",
            json!({
                "target": "macos",
                "target_selector": {
                    "Pid": 101
                },
                "focus_policy": "Auto",
                "x": 80.0,
                "y": 120.0,
                "width": 900.0,
                "height": 700.0
            }),
        )
        .await
        .unwrap();
    let switched = runtime
        .tools()
        .invoke(
            "switch-app",
            json!({
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
                "target": "macos",
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
    assert_eq!(
        moved_window["outcome"]["detail"],
        json!("moved window 42 to x=120 y=240 width=640 height=480")
    );
    assert_eq!(
        resized_window["outcome"]["detail"],
        json!("resized window 42 to x=120 y=240 width=800 height=600")
    );
    assert_eq!(
        set_window_bounds["outcome"]["detail"],
        json!("set window 42 bounds to x=80 y=120 width=900 height=700")
    );
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::MoveWindow { x: 120.0, y: 240.0 },
                    locator: None,
                    target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                    focus_policy: ActionFocusPolicy::Never,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::ResizeWindow {
                        width: 800.0,
                        height: 600.0,
                    },
                    locator: None,
                    target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
            (
                ActionRequest {
                    action: Action::SetWindowBounds {
                        bounds: Rect {
                            x: 80.0,
                            y: 120.0,
                            width: 900.0,
                            height: 700.0,
                        },
                    },
                    locator: None,
                    target_selector: Some(ActionTargetSelector::Pid(101)),
                    focus_policy: ActionFocusPolicy::Auto,
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
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
                    verifications: Vec::new(),
                },
                ExecContext {
                    target: "macos".into(),
                    session: None,
                    timeout_ms: Some(10_000),
                },
            ),
        ]
    );
}

#[tokio::test]
async fn action_tools_serialize_richer_action_outcomes() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 6,
        detail: Some("moved".into()),
        coordinates: Some(ActionCoordinates {
            point: Some(Point { x: 320.0, y: 240.0 }),
            from: None,
            to: None,
        }),
        target_app: Some(AppInfo {
            bundle_id: Some("com.apple.TextEdit".into()),
            name: "TextEdit".into(),
            pid: Some(101),
            is_running: true,
        }),
        target_window: Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 120.0,
                y: 80.0,
                width: 400.0,
                height: 300.0,
            }),
            is_focused: true,
            is_minimized: false,
        }),
        side_effects: vec![ActionSideEffect::MoveCursor],
        warnings: vec!["locator matched fallback element".into()],
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .unwrap();

    let moved = runtime
        .tools()
        .invoke(
            "move",
            json!({
                "target": "macos",
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

    assert_eq!(moved["outcome"]["detail"], json!("moved"));
    assert_eq!(
        moved["outcome"]["coordinates"]["point"],
        json!({
            "x": 320.0,
            "y": 240.0
        })
    );
    assert_eq!(
        moved["outcome"]["target_app"],
        json!({
            "bundle_id": "com.apple.TextEdit",
            "name": "TextEdit",
            "pid": 101,
            "is_running": true
        })
    );
    assert_eq!(
        moved["outcome"]["target_window"],
        json!({
            "id": 42,
            "title": "Draft",
            "app_name": "TextEdit",
            "bounds": {
                "x": 120.0,
                "y": 80.0,
                "width": 400.0,
                "height": 300.0
            },
            "is_focused": true,
            "is_minimized": false
        })
    );
    assert_eq!(
        moved["outcome"]["side_effects"],
        json!([
            {
                "kind": "MoveCursor"
            }
        ])
    );
    assert_eq!(
        moved["outcome"]["warnings"],
        json!(["locator matched fallback element"])
    );
}

#[tokio::test]
async fn action_tools_run_post_action_geometry_verification() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::WindowManagement]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("moved window 42 to x=120 y=240 width=640 height=480".into()),
        coordinates: None,
        target_app: None,
        target_window: Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 120.0,
                y: 240.0,
                width: 640.0,
                height: 480.0,
            }),
            is_focused: true,
            is_minimized: false,
        }),
        side_effects: vec![ActionSideEffect::MoveWindow {
            bounds: Rect {
                x: 120.0,
                y: 240.0,
                width: 640.0,
                height: 480.0,
            },
        }],
        warnings: Vec::new(),
    }));
    driver.push_query_result(Ok(QueryResult::Windows(vec![WindowInfo {
        id: 42.into(),
        title: Some("Draft".into()),
        app_name: Some("TextEdit".into()),
        bounds: Some(Rect {
            x: 120.0,
            y: 240.0,
            width: 640.0,
            height: 480.0,
        }),
        is_focused: true,
        is_minimized: false,
    }])));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let output = runtime
        .tools()
        .invoke(
            "move-window",
            json!({
                "target": "macos",
                "target_selector": {
                    "WindowId": 42
                },
                "x": 120.0,
                "y": 240.0,
                "verifications": ["Geometry"]
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        output["outcome"]["target_window"]["bounds"],
        json!({
            "x": 120.0,
            "y": 240.0,
            "width": 640.0,
            "height": 480.0
        })
    );
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::MoveWindow { x: 120.0, y: 240.0 },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::Geometry],
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
    assert_eq!(
        driver.query_calls().await,
        vec![(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test]
async fn action_tools_fail_when_post_action_focus_verification_misses_target_window() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::WindowManagement, Capability::InspectTree]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 5,
        detail: Some("focused window 42".into()),
        coordinates: None,
        target_app: None,
        target_window: Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 120.0,
                y: 80.0,
                width: 640.0,
                height: 480.0,
            }),
            is_focused: true,
            is_minimized: false,
        }),
        side_effects: vec![ActionSideEffect::FocusWindow],
        warnings: Vec::new(),
    }));
    driver.push_query_result(Ok(QueryResult::Windows(vec![WindowInfo {
        id: 42.into(),
        title: Some("Draft".into()),
        app_name: Some("TextEdit".into()),
        bounds: Some(Rect {
            x: 120.0,
            y: 80.0,
            width: 640.0,
            height: 480.0,
        }),
        is_focused: false,
        is_minimized: false,
    }])));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let error = runtime
        .tools()
        .invoke(
            "focus-window",
            json!({
                "target": "macos",
                "window_id": 42,
                "verifications": ["Focus"]
            }),
        )
        .await
        .unwrap_err();

    match error {
        OperatorError::Platform(message) => {
            assert_eq!(
                message,
                "post-action focus verification failed: window 42 is not focused"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::FocusWindow { id: 42.into() },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::Focus],
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
    assert_eq!(
        driver.query_calls().await,
        vec![(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test]
async fn runtime_act_rejects_unsupported_post_action_verifications() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::AppLifecycle, Capability::WindowManagement]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();
    let ctx = ExecContext {
        target: "macos".into(),
        session: None,
        timeout_ms: Some(10_000),
    };

    let cases = vec![
        (
            ActionRequest {
                action: Action::LaunchApp {
                    bundle_id_or_name: "Calculator".into(),
                },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::Focus],
            },
            "post-action Focus verification is not supported for launch-app",
        ),
        (
            ActionRequest {
                action: Action::CloseWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::WindowState],
            },
            "post-action WindowState verification is not supported for close-window",
        ),
        (
            ActionRequest {
                action: Action::MinimizeWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::Focus],
            },
            "post-action Focus verification is not supported for minimize-window",
        ),
        (
            ActionRequest {
                action: Action::MaximizeWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::WindowState],
            },
            "post-action WindowState verification is not supported for maximize-window",
        ),
    ];

    for (req, expected) in cases {
        let error = runtime.core().act(req, ctx.clone()).await.unwrap_err();
        match error {
            OperatorError::Platform(message) => assert_eq!(message, expected),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    assert!(driver.action_calls().await.is_empty());
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
            "move-window",
            "observe",
            "permissions-status",
            "press",
            "quit-app",
            "relaunch-app",
            "resize-window",
            "scroll",
            "set-window-bounds",
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
    assert!(click.input_schema["properties"]["verifications"].is_object());

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
    assert!(launch_app.input_schema["properties"]["verifications"].is_null());

    let close_window = specs
        .iter()
        .find(|spec| spec.name == "close-window")
        .unwrap();
    assert!(close_window.has_side_effects);
    assert_eq!(
        close_window.capabilities_required,
        &[Capability::WindowManagement]
    );
    assert!(close_window.input_schema["properties"]["target_selector"].is_object());
    assert!(close_window.input_schema["properties"]["focus_policy"].is_object());
    assert!(close_window.input_schema["properties"]["verifications"].is_null());

    let minimize_window = specs
        .iter()
        .find(|spec| spec.name == "minimize-window")
        .unwrap();
    assert!(minimize_window.has_side_effects);
    assert_eq!(
        minimize_window.capabilities_required,
        &[Capability::WindowManagement]
    );
    assert!(minimize_window.input_schema["properties"]["target_selector"].is_object());
    assert!(minimize_window.input_schema["properties"]["focus_policy"].is_object());
    assert_eq!(
        verification_enum_values(&minimize_window.input_schema),
        vec!["WindowState"]
    );

    let maximize_window = specs
        .iter()
        .find(|spec| spec.name == "maximize-window")
        .unwrap();
    assert!(maximize_window.has_side_effects);
    assert_eq!(
        maximize_window.capabilities_required,
        &[Capability::WindowManagement]
    );
    assert!(maximize_window.input_schema["properties"]["target_selector"].is_object());
    assert!(maximize_window.input_schema["properties"]["focus_policy"].is_object());
    assert!(maximize_window.input_schema["properties"]["verifications"].is_null());

    for tool_name in ["move-window", "resize-window", "set-window-bounds"] {
        let spec = specs.iter().find(|spec| spec.name == tool_name).unwrap();
        assert!(spec.has_side_effects);
        assert_eq!(spec.capabilities_required, &[Capability::WindowManagement]);
        assert!(spec.input_schema["properties"]["target_selector"].is_object());
        assert!(spec.input_schema["properties"]["focus_policy"].is_object());
        assert!(spec.input_schema["properties"]["verifications"].is_object());
    }

    let move_window = specs
        .iter()
        .find(|spec| spec.name == "move-window")
        .unwrap();
    assert!(move_window.input_schema["properties"]["x"].is_object());
    assert!(move_window.input_schema["properties"]["y"].is_object());

    let resize_window = specs
        .iter()
        .find(|spec| spec.name == "resize-window")
        .unwrap();
    assert!(resize_window.input_schema["properties"]["width"].is_object());
    assert!(resize_window.input_schema["properties"]["height"].is_object());

    let set_window_bounds = specs
        .iter()
        .find(|spec| spec.name == "set-window-bounds")
        .unwrap();
    assert!(set_window_bounds.input_schema["properties"]["x"].is_object());
    assert!(set_window_bounds.input_schema["properties"]["y"].is_object());
    assert!(set_window_bounds.input_schema["properties"]["width"].is_object());
    assert!(set_window_bounds.input_schema["properties"]["height"].is_object());

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
    assert!(move_spec.input_schema["properties"]["verifications"].is_object());

    let focus_window = specs
        .iter()
        .find(|spec| spec.name == "focus-window")
        .unwrap();
    assert!(focus_window.has_side_effects);
    assert_eq!(
        focus_window.capabilities_required,
        &[Capability::WindowManagement]
    );

    let list_windows = specs
        .iter()
        .find(|spec| spec.name == "list-windows")
        .unwrap();
    assert!(!list_windows.has_side_effects);
    assert_eq!(
        list_windows.capabilities_required,
        &[Capability::WindowQuery]
    );
    assert!(list_windows.input_schema["properties"]["app"].is_object());

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
    assert!(press.input_schema["properties"]["verifications"].is_object());

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
    assert!(type_spec.input_schema["properties"]["verifications"].is_object());

    let observe = specs.iter().find(|spec| spec.name == "observe").unwrap();
    assert!(!observe.has_side_effects);
    assert_eq!(observe.capabilities_required.len(), 0);
    assert!(observe.input_schema["properties"]["surface"].is_object());
    assert!(observe.input_schema["properties"]["include_screenshot"].is_object());
    assert!(observe.input_schema["properties"]["include_elements"].is_object());

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
    driver.push_action_result(Ok(successful_action_outcome("dragged", 16)));

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
                "target": "macos",
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
                target: "macos".into(),
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
    driver.push_action_result(Ok(successful_action_outcome("swiped", 15)));

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
                "target": "macos",
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
                target: "macos".into(),
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
