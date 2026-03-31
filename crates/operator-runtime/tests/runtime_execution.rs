use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionFocusPolicy, ActionOutcome, ActionRequest, AppInfo, Capability, CapabilitySet,
    ClickMode, DragMotion, DriverConfig, ElementId, ElementSource, ExecContext, FocusInfo,
    HealthStatus, ImageSizePx, Locator, ObserveRequest, ObserveResult, OperatorError,
    PermissionCheck, PermissionStatus, PermissionsReport, PlatformDriver, Point, QueryRequest,
    QueryResult, Rect, Surface, SurfaceKind, TargetId, UiElement,
};
use operator_runtime::{
    AuditEvent, AuditEventKind, EventSink, NamedTargetConfig, RuntimeBuilder, RuntimeConfig,
    SnapshotStore,
};
use operator_testkit::{test_snapshot, InMemorySnapshotStore, MockPlatformDriver};

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

#[tokio::test]
async fn runtime_rejects_missing_capabilities_before_driver_call() {
    let events = Arc::new(RecordingEventSink::default());
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture]),
    ));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .event_sink(events.clone())
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let error = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: Some("sess-1".into()),
                timeout_ms: None,
            },
        )
        .await
        .unwrap_err();

    match error {
        OperatorError::CapabilityNotSupported(Capability::PointerInput) => {}
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(driver.action_calls().await.is_empty());

    let events = events.events();
    assert!(matches!(
        events.as_slice(),
        [AuditEvent {
            kind: AuditEventKind::CapabilityDenied {
                tool,
                capability: Capability::PointerInput,
            },
            ..
        }] if tool == "act"
    ));
}

#[tokio::test]
async fn runtime_persists_snapshot_after_observe() {
    let events = Arc::new(RecordingEventSink::default());
    let store = Arc::new(InMemorySnapshotStore::new());
    let snapshot = test_snapshot("snap-1");
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    driver.push_observe_result(Ok(ObserveResult {
        snapshot: snapshot.clone(),
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store.clone())
        .event_sink(events.clone())
        .register_driver(driver)
        .build()
        .await
        .unwrap();

    let result = runtime
        .core()
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: true,
            },
            ExecContext {
                target: "macos".into(),
                session: Some("sess-2".into()),
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.snapshot, snapshot);
    assert_eq!(store.get(&snapshot.id).await.unwrap(), Some(snapshot));

    let events = events.events();
    assert!(matches!(
        &events[0].kind,
        AuditEventKind::ToolInvoked { tool, .. } if tool == "observe"
    ));
    assert!(matches!(
        &events[1].kind,
        AuditEventKind::ToolCompleted {
            tool,
            success: true,
            ..
        } if tool == "observe"
    ));
}

#[tokio::test]
async fn runtime_allows_screenshot_only_observe_on_capture_only_driver() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-capture-only");
    snapshot.elements.clear();
    snapshot.root_ids.clear();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture]),
    ));
    driver.push_observe_result(Ok(ObserveResult {
        snapshot: snapshot.clone(),
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store.clone())
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let result = runtime
        .core()
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: false,
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.snapshot, snapshot);
    assert_eq!(store.get(&snapshot.id).await.unwrap(), Some(snapshot));
    assert_eq!(driver.observe_calls().await.len(), 1);
}

#[tokio::test]
async fn runtime_allows_tree_only_observe_on_inspect_tree_only_driver() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let snapshot = test_snapshot("snap-tree-only");

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::InspectTree]),
    ));
    driver.push_observe_result(Ok(ObserveResult {
        snapshot: snapshot.clone(),
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store.clone())
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let result = runtime
        .core()
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: false,
                include_elements: true,
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.snapshot, snapshot);
    assert_eq!(store.get(&snapshot.id).await.unwrap(), Some(snapshot));
    assert_eq!(driver.observe_calls().await.len(), 1);
}

#[tokio::test]
async fn runtime_rejects_mixed_observe_when_driver_lacks_tree_inspection() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture]),
    ));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let error = runtime
        .core()
        .observe(
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
        .await
        .unwrap_err();

    match error {
        OperatorError::CapabilityNotSupported(Capability::InspectTree) => {}
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(driver.observe_calls().await.is_empty());
}

#[tokio::test]
async fn runtime_times_out_slow_driver_calls() {
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        default_target: TargetId("slow".into()),
        targets: BTreeMap::from([(
            "slow".into(),
            NamedTargetConfig {
                platform: "slow".into(),
                driver: "slow.system".into(),
                description: None,
                driver_config: DriverConfig::new(),
            },
        )]),
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .register_driver(Arc::new(SlowQueryDriver))
    .build()
    .await
    .unwrap();

    let error = runtime
        .core()
        .query(
            QueryRequest::ListWindows { app: None },
            ExecContext {
                target: "slow".into(),
                session: None,
                timeout_ms: Some(5),
            },
        )
        .await
        .unwrap_err();

    match error {
        OperatorError::Timeout { timeout_ms } => assert_eq!(timeout_ms, 5),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn runtime_rejects_drag_between_different_snapshots() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let error = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Drag {
                    from: Locator::SnapshotElement {
                        snapshot: "snap-1".into(),
                        element: "el-1".into(),
                    },
                    to: Locator::SnapshotElement {
                        snapshot: "snap-2".into(),
                        element: "el-2".into(),
                    },
                    motion: DragMotion::default(),
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(100),
            },
        )
        .await
        .unwrap_err();

    match error {
        OperatorError::Platform(message) => {
            assert_eq!(message, "drag: from/to must reference the same snapshot")
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(driver.action_calls().await.is_empty());
}

#[tokio::test]
async fn runtime_rejects_swipe_between_different_snapshots() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let error = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Swipe {
                    from: Locator::SnapshotElement {
                        snapshot: "snap-1".into(),
                        element: "el-1".into(),
                    },
                    to: Locator::SnapshotElement {
                        snapshot: "snap-2".into(),
                        element: "el-2".into(),
                    },
                    duration_ms: Some(250),
                    steps: Some(4.try_into().unwrap()),
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(100),
            },
        )
        .await
        .unwrap_err();

    match error {
        OperatorError::Platform(message) => {
            assert_eq!(message, "swipe: from/to must reference the same snapshot")
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(driver.action_calls().await.is_empty());
}

#[tokio::test]
async fn runtime_resolves_drag_snapshot_element_locators_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-drag");
    snapshot.elements.get_mut(&"el-1".into()).unwrap().bounds = Some(Rect {
        x: 40.0,
        y: 20.0,
        width: 60.0,
        height: 30.0,
    });
    snapshot.elements.insert(
        ElementId("el-2".into()),
        UiElement {
            id: ElementId("el-2".into()),
            role: "AXButton".into(),
            label: Some("drop target".into()),
            value: None,
            bounds: Some(Rect {
                x: 140.0,
                y: 80.0,
                width: 100.0,
                height: 40.0,
            }),
            enabled: Some(true),
            children: vec![],
            confidence: Some(1.0),
            source: ElementSource::Native,
        },
    );
    snapshot.root_ids.push(ElementId("el-2".into()));
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("dragged", 11)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Drag {
                    from: Locator::SnapshotElement {
                        snapshot: snapshot.id.clone(),
                        element: "el-1".into(),
                    },
                    to: Locator::SnapshotElement {
                        snapshot: snapshot.id.clone(),
                        element: "el-2".into(),
                    },
                    motion: DragMotion {
                        duration_ms: Some(300),
                        steps: Some(6.try_into().unwrap()),
                        modifiers: vec![],
                    },
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Drag {
                from: Locator::Coords(Point { x: 70.0, y: 35.0 }),
                to: Locator::Coords(Point { x: 190.0, y: 100.0 }),
                motion: DragMotion {
                    duration_ms: Some(300),
                    steps: Some(6.try_into().unwrap()),
                    modifiers: vec![],
                },
            },
            locator: None,
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_resolves_swipe_snapshot_element_locators_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-swipe");
    snapshot.elements.get_mut(&"el-1".into()).unwrap().bounds = Some(Rect {
        x: 20.0,
        y: 40.0,
        width: 50.0,
        height: 20.0,
    });
    snapshot.elements.insert(
        ElementId("el-2".into()),
        UiElement {
            id: ElementId("el-2".into()),
            role: "AXButton".into(),
            label: Some("swipe target".into()),
            value: None,
            bounds: Some(Rect {
                x: 120.0,
                y: 44.0,
                width: 80.0,
                height: 24.0,
            }),
            enabled: Some(true),
            children: vec![],
            confidence: Some(1.0),
            source: ElementSource::Native,
        },
    );
    snapshot.root_ids.push(ElementId("el-2".into()));
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("swiped", 14)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Swipe {
                    from: Locator::SnapshotElement {
                        snapshot: snapshot.id.clone(),
                        element: "el-1".into(),
                    },
                    to: Locator::SnapshotElement {
                        snapshot: snapshot.id.clone(),
                        element: "el-2".into(),
                    },
                    duration_ms: Some(240),
                    steps: Some(4.try_into().unwrap()),
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Swipe {
                from: Locator::Coords(Point { x: 45.0, y: 50.0 }),
                to: Locator::Coords(Point { x: 160.0, y: 56.0 }),
                duration_ms: Some(240),
                steps: Some(4.try_into().unwrap()),
            },
            locator: None,
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_resolves_snapshot_element_locator_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-click");
    snapshot.elements.get_mut(&"el-1".into()).unwrap().bounds = Some(Rect {
        x: 40.0,
        y: 20.0,
        width: 60.0,
        height: 30.0,
    });
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("clicked", 7)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::SnapshotElement {
                    snapshot: snapshot.id.clone(),
                    element: "el-1".into(),
                }),
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Click {
                mode: ClickMode::Left,
            },
            locator: Some(Locator::Coords(Point { x: 70.0, y: 35.0 })),
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_resolves_scroll_snapshot_element_locator_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-scroll");
    snapshot.elements.get_mut(&"el-1".into()).unwrap().bounds = Some(Rect {
        x: 20.0,
        y: 40.0,
        width: 80.0,
        height: 20.0,
    });
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("scrolled", 9)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Scroll {
                    delta_x: 0.0,
                    delta_y: -120.0,
                },
                locator: Some(Locator::SnapshotElement {
                    snapshot: snapshot.id.clone(),
                    element: "el-1".into(),
                }),
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Scroll {
                delta_x: 0.0,
                delta_y: -120.0,
            },
            locator: Some(Locator::Coords(Point { x: 60.0, y: 50.0 })),
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_resolves_snapshot_coords_locator_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-coords");
    snapshot.metadata.capture_bounds = Some(Rect {
        x: 400.0,
        y: 240.0,
        width: 300.0,
        height: 640.0,
    });
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("clicked", 7)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::SnapshotCoords {
                    snapshot: snapshot.id.clone(),
                    point: Point { x: 152.0, y: 772.0 },
                }),
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Click {
                mode: ClickMode::Left,
            },
            locator: Some(Locator::Coords(Point {
                x: 552.0,
                y: 1012.0
            })),
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_resolves_snapshot_normalized_coords_locator_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-normalized");
    snapshot.metadata.capture_bounds = Some(Rect {
        x: 400.0,
        y: 240.0,
        width: 300.0,
        height: 640.0,
    });
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("clicked", 7)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::SnapshotNormalizedCoords {
                    snapshot: snapshot.id.clone(),
                    point: Point { x: 152.0, y: 772.0 },
                    basis: 1000.0,
                }),
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Click {
                mode: ClickMode::Left,
            },
            locator: Some(Locator::Coords(Point {
                x: 445.6,
                y: 734.08,
            })),
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_resolves_snapshot_pixel_coords_locator_before_driver_call() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let mut snapshot = test_snapshot("snap-pixels");
    snapshot.metadata.capture_bounds = Some(Rect {
        x: 338.0,
        y: 216.0,
        width: 230.0,
        height: 408.0,
    });
    snapshot.metadata.image_size_px = Some(ImageSizePx {
        width: 460,
        height: 816,
    });
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("clicked", 7)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::SnapshotPixelCoords {
                    snapshot: snapshot.id.clone(),
                    point: Point { x: 176.0, y: 314.0 },
                }),
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert!(outcome.success);

    let calls = driver.action_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0,
        ActionRequest {
            action: Action::Click {
                mode: ClickMode::Left,
            },
            locator: Some(Locator::Coords(Point { x: 426.0, y: 373.0 })),
            ..default_action_request()
        }
    );
}

#[tokio::test]
async fn runtime_rejects_snapshot_coords_without_capture_bounds() {
    let store = Arc::new(InMemorySnapshotStore::new());
    let snapshot = test_snapshot("snap-missing-bounds");
    store.save(&snapshot).await.unwrap();

    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(store)
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let error = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::SnapshotCoords {
                    snapshot: "snap-missing-bounds".into(),
                    point: Point { x: 152.0, y: 772.0 },
                }),
                ..default_action_request()
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        OperatorError::Platform(message)
            if message.contains("has no capture bounds for coordinate normalization")
    ));
    assert!(driver.action_calls().await.is_empty());
}

#[tokio::test]
async fn runtime_focus_verification_accepts_matching_bundle_id_when_app_name_differs() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::PointerInput,
            Capability::InspectTree,
            Capability::WindowManagement,
        ]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 7,
        detail: Some("clicked".into()),
        coordinates: Some(operator_core::ActionCoordinates {
            point: Some(Point { x: 70.0, y: 651.0 }),
            from: None,
            to: None,
        }),
        target_app: Some(AppInfo {
            bundle_id: Some("com.apple.calculator".into()),
            name: "Calculator".into(),
            pid: Some(42),
            is_running: true,
        }),
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }));
    driver.push_query_result(Ok(QueryResult::Focus(Some(FocusInfo {
        role: "AXButton".into(),
        label: Some("1".into()),
        bounds: Some(Rect {
            x: 348.0,
            y: 565.0,
            width: 48.0,
            height: 48.0,
        }),
        bundle_id: Some("com.apple.calculator".into()),
        app_name: Some("计算器".into()),
    }))));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let outcome = runtime
        .core()
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::Coords(Point { x: 70.0, y: 651.0 })),
                target_selector: Some(operator_core::ActionTargetSelector::App(
                    "Calculator".into(),
                )),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![operator_core::ActionVerification::Focus],
            },
            ExecContext {
                target: "macos".into(),
                session: None,
                timeout_ms: Some(250),
            },
        )
        .await
        .expect("bundle id should satisfy focus verification even with localized app name");

    assert!(outcome.success);
    assert_eq!(driver.query_calls().await.len(), 1);
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

struct SlowQueryDriver;

#[async_trait]
impl PlatformDriver for SlowQueryDriver {
    fn platform_id(&self) -> &'static str {
        "slow"
    }

    fn driver_id(&self) -> &str {
        "slow.system"
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new([Capability::WindowQuery])
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        Ok(HealthStatus {
            healthy: true,
            message: None,
            permissions: PermissionsReport::new([
                PermissionCheck::new("accessibility", "Accessibility", PermissionStatus::Granted),
                PermissionCheck::new("system_events", "System Events", PermissionStatus::Granted),
                PermissionCheck::new(
                    "screen_recording",
                    "Screen Recording",
                    PermissionStatus::Granted,
                ),
            ]),
        })
    }

    async fn observe(
        &self,
        _: ObserveRequest,
        _: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        Err(OperatorError::Platform("observe unused in test".into()))
    }

    async fn query(&self, _: QueryRequest, _: &ExecContext) -> Result<QueryResult, OperatorError> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(QueryResult::Capabilities(CapabilitySet::new([])))
    }

    async fn act(&self, _: ActionRequest, _: &ExecContext) -> Result<ActionOutcome, OperatorError> {
        Err(OperatorError::Platform("act unused in test".into()))
    }
}
