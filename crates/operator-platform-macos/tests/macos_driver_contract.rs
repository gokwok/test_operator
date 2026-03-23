use std::{collections::HashMap, sync::Mutex};

use operator_core::{
    Action, ActionRequest, AppInfo, ArtifactId, Capability, ClickMode, DragModifier, DragMotion,
    ElementId, ElementSource, ExecContext, FocusInfo, Locator, ObserveRequest, OperatorError,
    PermissionStatus, PermissionsReport, PlatformDriver, Point, QueryRequest, QueryResult, Rect,
    Surface, SurfaceKind, UiElement, WindowId, WindowInfo,
};
use operator_platform_macos::{
    AppService, CaptureProvider, CaptureResult, InputSynthesizer, InspectResult, MacosDriver,
    PermissionReader, TreeInspector,
};

#[test]
fn macos_driver_declares_expected_capabilities() {
    let driver = MacosDriver::new(StubAppService::default(), StubPermissionReader::granted());
    let capabilities = driver.capabilities();

    assert!(capabilities.supports(&Capability::AppLifecycle));
    assert!(capabilities.supports(&Capability::WindowManagement));
    assert!(capabilities.supports(&Capability::Permissions));
    assert!(capabilities.supports(&Capability::Capture));
    assert!(capabilities.supports(&Capability::InspectTree));
    assert!(capabilities.supports(&Capability::PointerInput));
    assert!(capabilities.supports(&Capability::KeyboardInput));
}

#[tokio::test]
async fn observe_frontmost_returns_snapshot_with_metadata() {
    let driver = MacosDriver::with_observe(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("artifact-frontmost.png".into()),
            display_scale: Some(2.0),
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([(
                ElementId("ax-0".into()),
                UiElement {
                    id: ElementId("ax-0".into()),
                    role: "AXButton".into(),
                    label: Some("Continue".into()),
                    value: None,
                    bounds: Some(Rect {
                        x: 10.0,
                        y: 20.0,
                        width: 100.0,
                        height: 44.0,
                    }),
                    enabled: Some(true),
                    children: vec![],
                    confidence: Some(1.0),
                    source: ElementSource::Native,
                },
            )]),
            root_ids: vec![ElementId("ax-0".into())],
        }),
    );

    let surface = Surface {
        kind: SurfaceKind::Frontmost,
    };
    let observed = driver
        .observe(
            ObserveRequest {
                surface: surface.clone(),
                include_screenshot: true,
                include_elements: true,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(observed.snapshot.target, "local:macos".into());
    assert_eq!(observed.snapshot.surface, surface);
    assert_eq!(
        observed.snapshot.image_artifact,
        Some(ArtifactId("artifact-frontmost.png".into()))
    );
    assert_eq!(observed.snapshot.metadata.platform, "macos");
    assert_eq!(observed.snapshot.metadata.display_scale, Some(2.0));
    assert!(!observed.snapshot.id.to_string().is_empty());
    assert!(observed.snapshot.metadata.capture_duration_ms < 1_000);
    assert_eq!(observed.snapshot.root_ids, vec![ElementId("ax-0".into())]);
    assert_eq!(
        observed.snapshot.elements[&ElementId("ax-0".into())]
            .label
            .as_deref(),
        Some("Continue")
    );
    assert_eq!(
        driver.capture_provider().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Frontmost,
        }]
    );
    assert_eq!(
        driver.tree_inspector().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Frontmost,
        }]
    );
}

#[tokio::test]
async fn permissions_query_returns_report() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(PermissionsReport {
            screen_recording: PermissionStatus::Denied,
            accessibility: PermissionStatus::Granted,
        }),
    );

    let result = driver
        .query(QueryRequest::PermissionsStatus, &exec_context())
        .await
        .unwrap();

    assert_eq!(
        result,
        QueryResult::Permissions(PermissionsReport {
            screen_recording: PermissionStatus::Denied,
            accessibility: PermissionStatus::Granted,
        })
    );
}

#[tokio::test]
async fn list_apps_and_windows_queries_forward_to_services() {
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            windows: vec![WindowInfo {
                id: 42.into(),
                title: Some("Notes.txt".into()),
                app_name: Some("TextEdit".into()),
                bounds: None,
                is_focused: true,
                is_minimized: false,
            }],
            focus: None,
            launched: Mutex::new(Vec::new()),
            focused_windows: Mutex::new(Vec::new()),
            last_window_filter: Mutex::new(None),
        },
        StubPermissionReader::granted(),
    );

    let apps = driver
        .query(QueryRequest::ListApps, &exec_context())
        .await
        .unwrap();
    let windows = driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(
        apps,
        QueryResult::Apps(vec![AppInfo {
            bundle_id: Some("com.apple.TextEdit".into()),
            name: "TextEdit".into(),
            pid: Some(101),
            is_running: true,
        }])
    );
    assert_eq!(
        windows,
        QueryResult::Windows(vec![WindowInfo {
            id: 42.into(),
            title: Some("Notes.txt".into()),
            app_name: Some("TextEdit".into()),
            bounds: None,
            is_focused: true,
            is_minimized: false,
        }])
    );
    assert_eq!(
        driver.app_service().last_window_filter(),
        Some("TextEdit".to_string())
    );
}

#[tokio::test]
async fn get_focus_query_returns_focus_info() {
    let focus = FocusInfo {
        role: "AXTextField".into(),
        label: Some("Search".into()),
        bounds: Some(Rect {
            x: 120.0,
            y: 80.0,
            width: 300.0,
            height: 28.0,
        }),
        app_name: Some("Safari".into()),
    };
    let driver = MacosDriver::new(
        StubAppService {
            focus: Some(focus.clone()),
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let result = driver
        .query(QueryRequest::GetFocus, &exec_context())
        .await
        .unwrap();

    assert_eq!(result, QueryResult::Focus(Some(focus)));
}

#[tokio::test]
async fn launch_app_action_returns_successful_outcome() {
    let driver = MacosDriver::new(StubAppService::default(), StubPermissionReader::granted());

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::LaunchApp {
                    bundle_id_or_name: "TextEdit".into(),
                },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        driver.app_service().launched_apps(),
        vec!["TextEdit".to_string()]
    );
}

#[tokio::test]
async fn focus_window_action_returns_successful_outcome() {
    let driver = MacosDriver::new(StubAppService::default(), StubPermissionReader::granted());

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::FocusWindow { id: 42.into() },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("focused window 42"));
    assert_eq!(
        driver.app_service().focused_windows(),
        vec![WindowId::from(42)]
    );
}

#[tokio::test]
async fn click_action_resolves_text_locator_to_button_center() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([(
                ElementId("ax-button".into()),
                UiElement {
                    id: ElementId("ax-button".into()),
                    role: "AXButton".into(),
                    label: Some("Submit".into()),
                    value: None,
                    bounds: Some(Rect {
                        x: 100.0,
                        y: 40.0,
                        width: 80.0,
                        height: 20.0,
                    }),
                    enabled: Some(true),
                    children: vec![],
                    confidence: Some(1.0),
                    source: ElementSource::Native,
                },
            )]),
            root_ids: vec![ElementId("ax-button".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Right,
                },
                locator: Some(Locator::Text("submit".into())),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("right-clicked"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 140.0, y: 50.0 }),
            mode: ClickMode::Right,
        }]
    );
}

#[tokio::test]
async fn click_action_without_locator_uses_current_cursor_position() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("clicked"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: None,
            mode: ClickMode::Left,
        }]
    );
}

#[tokio::test]
async fn click_action_supports_double_click_mode() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([(
                ElementId("ax-item".into()),
                UiElement {
                    id: ElementId("ax-item".into()),
                    role: "AXButton".into(),
                    label: Some("Open".into()),
                    value: None,
                    bounds: Some(Rect {
                        x: 20.0,
                        y: 30.0,
                        width: 40.0,
                        height: 20.0,
                    }),
                    enabled: Some(true),
                    children: vec![],
                    confidence: Some(1.0),
                    source: ElementSource::Native,
                },
            )]),
            root_ids: vec![ElementId("ax-item".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Double,
                },
                locator: Some(Locator::Text("open".into())),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("double-clicked"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 40.0, y: 40.0 }),
            mode: ClickMode::Double,
        }]
    );
}

#[tokio::test]
async fn type_action_clicks_role_target_before_typing() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([
                (
                    ElementId("ax-field-0".into()),
                    UiElement {
                        id: ElementId("ax-field-0".into()),
                        role: "AXTextField".into(),
                        label: Some("Ignored".into()),
                        value: None,
                        bounds: Some(Rect {
                            x: 10.0,
                            y: 10.0,
                            width: 100.0,
                            height: 20.0,
                        }),
                        enabled: Some(true),
                        children: vec![],
                        confidence: Some(1.0),
                        source: ElementSource::Native,
                    },
                ),
                (
                    ElementId("ax-field-1".into()),
                    UiElement {
                        id: ElementId("ax-field-1".into()),
                        role: "AXTextField".into(),
                        label: Some("Primary".into()),
                        value: None,
                        bounds: Some(Rect {
                            x: 200.0,
                            y: 60.0,
                            width: 120.0,
                            height: 24.0,
                        }),
                        enabled: Some(true),
                        children: vec![],
                        confidence: Some(1.0),
                        source: ElementSource::Native,
                    },
                ),
            ]),
            root_ids: vec![
                ElementId("ax-field-0".into()),
                ElementId("ax-field-1".into()),
            ],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Type {
                    text: "hello operator".into(),
                },
                locator: Some(Locator::Role {
                    role: "AXTextField".into(),
                    index: 1,
                }),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        input.calls(),
        vec![
            RecordedInput::Click {
                point: Some(Point { x: 260.0, y: 72.0 }),
                mode: ClickMode::Left,
            },
            RecordedInput::TypeText("hello operator".into()),
        ]
    );
}

#[tokio::test]
async fn scroll_action_returns_successful_outcome() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Scroll {
                    delta_x: 0.0,
                    delta_y: -12.0,
                },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("scrolled"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Scroll {
            point: None,
            delta_x: 0.0,
            delta_y: -12.0,
        }]
    );
}

#[tokio::test]
async fn scroll_action_resolves_text_locator_before_scrolling() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([(
                ElementId("ax-scroll-target".into()),
                UiElement {
                    id: ElementId("ax-scroll-target".into()),
                    role: "AXScrollArea".into(),
                    label: Some("Results".into()),
                    value: None,
                    bounds: Some(Rect {
                        x: 120.0,
                        y: 160.0,
                        width: 80.0,
                        height: 40.0,
                    }),
                    enabled: Some(true),
                    children: vec![],
                    confidence: Some(1.0),
                    source: ElementSource::Native,
                },
            )]),
            root_ids: vec![ElementId("ax-scroll-target".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Scroll {
                    delta_x: 0.0,
                    delta_y: -12.0,
                },
                locator: Some(Locator::Text("Results".into())),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("scrolled"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Scroll {
            point: Some(Point { x: 160.0, y: 180.0 }),
            delta_x: 0.0,
            delta_y: -12.0,
        }]
    );
}

#[tokio::test]
async fn drag_action_resolves_between_locators_and_returns_successful_outcome() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([
                (
                    ElementId("ax-start".into()),
                    UiElement {
                        id: ElementId("ax-start".into()),
                        role: "AXStaticText".into(),
                        label: Some("Drag start".into()),
                        value: None,
                        bounds: Some(Rect {
                            x: 10.0,
                            y: 20.0,
                            width: 40.0,
                            height: 20.0,
                        }),
                        enabled: Some(true),
                        children: vec![],
                        confidence: Some(1.0),
                        source: ElementSource::Native,
                    },
                ),
                (
                    ElementId("ax-drop".into()),
                    UiElement {
                        id: ElementId("ax-drop".into()),
                        role: "AXButton".into(),
                        label: Some("Drop here".into()),
                        value: None,
                        bounds: Some(Rect {
                            x: 100.0,
                            y: 120.0,
                            width: 80.0,
                            height: 30.0,
                        }),
                        enabled: Some(true),
                        children: vec![],
                        confidence: Some(1.0),
                        source: ElementSource::Native,
                    },
                ),
            ]),
            root_ids: vec![ElementId("ax-start".into()), ElementId("ax-drop".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Drag {
                    from: Locator::Text("drag start".into()),
                    to: Locator::Role {
                        role: "AXButton".into(),
                        index: 0,
                    },
                    motion: DragMotion {
                        duration_ms: Some(300),
                        steps: Some(6.try_into().unwrap()),
                        modifiers: vec![DragModifier::Command, DragModifier::Shift],
                    },
                },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("dragged"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Drag {
            from: Point { x: 30.0, y: 30.0 },
            to: Point { x: 140.0, y: 135.0 },
            motion: DragMotion {
                duration_ms: Some(300),
                steps: Some(6.try_into().unwrap()),
                modifiers: vec![DragModifier::Command, DragModifier::Shift],
            },
        }]
    );
}

#[tokio::test]
async fn hotkey_action_returns_successful_outcome() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Hotkey {
                    keys: vec!["command".into(), "shift".into(), "p".into()],
                },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("sent hotkey"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Hotkey(vec![
            "command".into(),
            "shift".into(),
            "p".into()
        ])]
    );
}

#[tokio::test]
async fn press_action_returns_successful_outcome() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Press {
                    key: "down".into(),
                    count: 3.try_into().unwrap(),
                },
                locator: None,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("pressed down 3 times"));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Press {
            key: "down".into(),
            count: 3,
        }]
    );
}

#[tokio::test]
async fn health_check_requires_accessibility_for_ready_status() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(PermissionsReport {
            screen_recording: PermissionStatus::NotDetermined,
            accessibility: PermissionStatus::Denied,
        }),
    );

    let health = driver.health_check().await.unwrap();

    assert!(!health.healthy);
    assert_eq!(
        health.message.as_deref(),
        Some("Accessibility permission is required for macOS automation.")
    );
    assert_eq!(health.permissions.accessibility, PermissionStatus::Denied);
}

#[tokio::test]
async fn health_check_requires_screen_recording_for_capture_readiness() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(PermissionsReport {
            screen_recording: PermissionStatus::Denied,
            accessibility: PermissionStatus::Granted,
        }),
    );

    let health = driver.health_check().await.unwrap();

    assert!(!health.healthy);
    assert_eq!(
        health.message.as_deref(),
        Some("Screen Recording permission is required for macOS capture.")
    );
    assert_eq!(
        health.permissions.screen_recording,
        PermissionStatus::Denied
    );
}

fn exec_context() -> ExecContext {
    ExecContext {
        target: "local:macos".into(),
        session: None,
        timeout_ms: Some(500),
    }
}

#[derive(Default)]
struct StubAppService {
    apps: Vec<AppInfo>,
    windows: Vec<WindowInfo>,
    focus: Option<FocusInfo>,
    launched: Mutex<Vec<String>>,
    focused_windows: Mutex<Vec<WindowId>>,
    last_window_filter: Mutex<Option<String>>,
}

impl StubAppService {
    fn launched_apps(&self) -> Vec<String> {
        self.launched.lock().unwrap().clone()
    }

    fn last_window_filter(&self) -> Option<String> {
        self.last_window_filter.lock().unwrap().clone()
    }

    fn focused_windows(&self) -> Vec<WindowId> {
        self.focused_windows.lock().unwrap().clone()
    }
}

impl AppService for StubAppService {
    fn list_apps(&self) -> Result<Vec<AppInfo>, OperatorError> {
        Ok(self.apps.clone())
    }

    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError> {
        *self.last_window_filter.lock().unwrap() = app.map(str::to_string);
        Ok(self.windows.clone())
    }

    fn get_focus(&self) -> Result<Option<FocusInfo>, OperatorError> {
        Ok(self.focus.clone())
    }

    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.launched
            .lock()
            .unwrap()
            .push(bundle_id_or_name.to_string());
        Ok(())
    }

    fn focus_window(&self, id: WindowId) -> Result<(), OperatorError> {
        self.focused_windows.lock().unwrap().push(id);
        Ok(())
    }
}

struct StubPermissionReader {
    report: PermissionsReport,
}

impl StubPermissionReader {
    fn granted() -> Self {
        Self::with_report(PermissionsReport {
            screen_recording: PermissionStatus::Granted,
            accessibility: PermissionStatus::Granted,
        })
    }

    fn with_report(report: PermissionsReport) -> Self {
        Self { report }
    }
}

impl PermissionReader for StubPermissionReader {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError> {
        Ok(self.report.clone())
    }
}

struct StubCaptureProvider {
    result: CaptureResult,
    requested_surfaces: Mutex<Vec<Surface>>,
}

impl StubCaptureProvider {
    fn with_result(result: CaptureResult) -> Self {
        Self {
            result,
            requested_surfaces: Mutex::new(Vec::new()),
        }
    }

    fn requested_surfaces(&self) -> Vec<Surface> {
        self.requested_surfaces.lock().unwrap().clone()
    }
}

impl CaptureProvider for StubCaptureProvider {
    fn capture(&self, surface: &Surface) -> Result<CaptureResult, OperatorError> {
        self.requested_surfaces
            .lock()
            .unwrap()
            .push(surface.clone());
        Ok(self.result.clone())
    }
}

struct StubTreeInspector {
    result: InspectResult,
    requested_surfaces: Mutex<Vec<Surface>>,
}

impl StubTreeInspector {
    fn with_result(result: InspectResult) -> Self {
        Self {
            result,
            requested_surfaces: Mutex::new(Vec::new()),
        }
    }

    fn requested_surfaces(&self) -> Vec<Surface> {
        self.requested_surfaces.lock().unwrap().clone()
    }
}

impl TreeInspector for StubTreeInspector {
    fn inspect(&self, surface: &Surface) -> Result<InspectResult, OperatorError> {
        self.requested_surfaces
            .lock()
            .unwrap()
            .push(surface.clone());
        Ok(self.result.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RecordedInput {
    Click {
        point: Option<Point>,
        mode: ClickMode,
    },
    Drag {
        from: Point,
        to: Point,
        motion: DragMotion,
    },
    Hotkey(Vec<String>),
    Press {
        key: String,
        count: u32,
    },
    Scroll {
        point: Option<Point>,
        delta_x: f64,
        delta_y: f64,
    },
    TypeText(String),
}

#[derive(Default, Clone)]
struct StubInputSynthesizer {
    calls: std::sync::Arc<Mutex<Vec<RecordedInput>>>,
}

impl StubInputSynthesizer {
    fn calls(&self) -> Vec<RecordedInput> {
        self.calls.lock().unwrap().clone()
    }
}

impl InputSynthesizer for StubInputSynthesizer {
    fn click(&self, point: Option<Point>, mode: ClickMode) -> Result<(), OperatorError> {
        self.calls
            .lock()
            .unwrap()
            .push(RecordedInput::Click { point, mode });
        Ok(())
    }

    fn drag(&self, from: Point, to: Point, motion: &DragMotion) -> Result<(), OperatorError> {
        self.calls.lock().unwrap().push(RecordedInput::Drag {
            from,
            to,
            motion: motion.clone(),
        });
        Ok(())
    }

    fn hotkey(&self, keys: &[String]) -> Result<(), OperatorError> {
        self.calls
            .lock()
            .unwrap()
            .push(RecordedInput::Hotkey(keys.to_vec()));
        Ok(())
    }

    fn press(&self, key: &str, count: u32) -> Result<(), OperatorError> {
        self.calls.lock().unwrap().push(RecordedInput::Press {
            key: key.to_string(),
            count,
        });
        Ok(())
    }

    fn scroll(
        &self,
        point: Option<Point>,
        delta_x: f64,
        delta_y: f64,
    ) -> Result<(), OperatorError> {
        self.calls.lock().unwrap().push(RecordedInput::Scroll {
            point,
            delta_x,
            delta_y,
        });
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), OperatorError> {
        self.calls
            .lock()
            .unwrap()
            .push(RecordedInput::TypeText(text.to_string()));
        Ok(())
    }
}
