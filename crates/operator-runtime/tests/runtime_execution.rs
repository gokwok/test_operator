use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, CapabilitySet, ClickMode, DragMotion,
    ElementId, ElementSource, ExecContext, HealthStatus, Locator, ObserveRequest, ObserveResult,
    OperatorError, PermissionStatus, PermissionsReport, PlatformDriver, Point, QueryRequest,
    QueryResult, Rect, Surface, SurfaceKind, UiElement,
};
use operator_runtime::{
    AuditEvent, AuditEventKind, EventSink, RuntimeBuilder, RuntimeConfig, SnapshotStore,
};
use operator_testkit::{test_snapshot, InMemorySnapshotStore, MockPlatformDriver};

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
            },
            ExecContext {
                target: "local:macos".into(),
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
                target: "local:macos".into(),
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
async fn runtime_times_out_slow_driver_calls() {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
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
                target: "local:slow".into(),
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
            },
            ExecContext {
                target: "local:macos".into(),
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
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 11,
        detail: Some("dragged".into()),
    }));

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
            },
            ExecContext {
                target: "local:macos".into(),
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
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 7,
        detail: Some("clicked".into()),
    }));

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
            },
            ExecContext {
                target: "local:macos".into(),
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
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 9,
        detail: Some("scrolled".into()),
    }));

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
            },
            ExecContext {
                target: "local:macos".into(),
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
        }
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

struct SlowQueryDriver;

#[async_trait]
impl PlatformDriver for SlowQueryDriver {
    fn platform_id(&self) -> &'static str {
        "slow"
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new([Capability::WindowManagement])
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        Ok(HealthStatus {
            healthy: true,
            message: None,
            permissions: PermissionsReport {
                screen_recording: PermissionStatus::Granted,
                accessibility: PermissionStatus::Granted,
            },
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
