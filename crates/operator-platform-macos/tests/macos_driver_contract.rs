use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Once,
    },
};

use operator_core::{
    Action, ActionCoordinates, ActionFocusPolicy, ActionRequest, ActionSideEffect,
    ActionTargetSelector, AppInfo, AppListMode, ArtifactId, Capability, ClickMode, DragModifier,
    DragMotion, ElementId, ElementSource, ExecContext, FocusInfo, Locator, ObserveRequest,
    OperatorError, PermissionCheck, PermissionStatus, PermissionsReport, PlatformDriver, Point,
    QueryRequest, QueryResult, Rect, Surface, SurfaceKind, TypeTrailingKey, UiElement, WindowId,
    WindowInfo,
};
use operator_platform_macos::{
    AppService, CaptureProvider, CaptureResult, InputSynthesizer, InspectResult, MacosDriver,
    PermissionReader, TreeInspector,
};

const ACTION_EFFECTS_DRY_RUN_ENV: &str = "OPERATOR_ACTION_EFFECTS_DRY_RUN";

#[test]
fn macos_driver_declares_expected_capabilities() {
    let driver = MacosDriver::new(StubAppService::default(), StubPermissionReader::granted());
    let capabilities = driver.capabilities();

    assert!(capabilities.supports(&Capability::AppLifecycle));
    assert!(capabilities.supports(&Capability::WindowQuery));
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
        StubAppService {
            windows: vec![WindowInfo {
                id: 42.into(),
                title: Some("Continue".into()),
                app_name: Some("Preview".into()),
                bounds: Some(Rect {
                    x: 10.0,
                    y: 20.0,
                    width: 100.0,
                    height: 44.0,
                }),
                is_focused: true,
                is_minimized: false,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("artifact-frontmost.png".into()),
            display_scale: Some(2.0),
            capture_bounds: None,
            image_size_px: None,
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
            kind: SurfaceKind::Window { id: 42.into() },
        }]
    );
    assert_eq!(
        driver.tree_inspector().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window { id: 42.into() },
        }]
    );
}

#[tokio::test]
async fn observe_frontmost_prefers_focused_window_over_tiny_auxiliary_window() {
    let focused_window = WindowInfo {
        id: 7.into(),
        title: Some("Calculator".into()),
        app_name: Some("Calculator".into()),
        bounds: Some(Rect {
            x: 338.0,
            y: 216.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_observe(
        StubAppService {
            windows: vec![
                WindowInfo {
                    id: 1.into(),
                    title: None,
                    app_name: Some("Codex".into()),
                    bounds: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1470.0,
                        height: 33.0,
                    }),
                    is_focused: false,
                    is_minimized: false,
                },
                focused_window.clone(),
            ],
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("artifact-frontmost.png".into()),
            display_scale: Some(2.0),
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
    );

    let observed = driver
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: true,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(
        driver.capture_provider().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window {
                id: focused_window.id,
            },
        }]
    );
    assert_eq!(
        driver.tree_inspector().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window {
                id: focused_window.id,
            },
        }]
    );
    assert_eq!(
        observed.snapshot.metadata.capture_bounds,
        focused_window.bounds
    );
}

#[tokio::test]
async fn observe_frontmost_routes_high_bit_window_ids_through_region_capture_and_window_tree_surface(
) {
    let synthetic_window = WindowInfo {
        id: WindowId(1 << 63 | 42),
        title: Some("计算器".into()),
        app_name: Some("Calculator".into()),
        bounds: Some(Rect {
            x: 535.0,
            y: 260.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_observe(
        StubAppService {
            windows: vec![synthetic_window.clone()],
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("artifact-frontmost.png".into()),
            display_scale: Some(2.0),
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
    );

    let observed = driver
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: true,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(
        driver.capture_provider().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Region {
                rect: synthetic_window.bounds.unwrap(),
            },
        }]
    );
    assert_eq!(
        driver.tree_inspector().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window {
                id: synthetic_window.id,
            },
        }]
    );
    assert_eq!(
        observed.snapshot.metadata.capture_bounds,
        synthetic_window.bounds
    );
}

#[tokio::test]
async fn observe_frontmost_screenshot_only_uses_frontmost_window_lookup() {
    let frontmost_window = WindowInfo {
        id: 21.into(),
        title: Some("Calculator".into()),
        app_name: Some("Calculator".into()),
        bounds: Some(Rect {
            x: 338.0,
            y: 216.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_observe(
        StubAppService {
            list_windows_error: Some("full window enumeration should not run".into()),
            frontmost_windows: Some(vec![frontmost_window.clone()]),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("artifact-frontmost.png".into()),
            display_scale: Some(2.0),
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
    );

    let observed = driver
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: false,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(
        driver.capture_provider().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window {
                id: frontmost_window.id,
            },
        }]
    );
    assert_eq!(driver.app_service().frontmost_window_query_count(), 1);
    assert_eq!(observed.snapshot.metadata.display_scale, Some(2.0));
    assert_eq!(
        observed.snapshot.metadata.capture_bounds,
        frontmost_window.bounds
    );
}

#[tokio::test]
async fn observe_frontmost_with_elements_uses_frontmost_window_lookup() {
    let frontmost_window = WindowInfo {
        id: 21.into(),
        title: Some("Calculator".into()),
        app_name: Some("Calculator".into()),
        bounds: Some(Rect {
            x: 338.0,
            y: 216.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_observe(
        StubAppService {
            frontmost_windows: Some(vec![frontmost_window.clone()]),
            list_windows_error: Some("full window enumeration should not run".into()),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("artifact-frontmost.png".into()),
            display_scale: Some(2.0),
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }),
    );

    driver
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: true,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(driver.app_service().frontmost_window_query_count(), 1);
    assert_eq!(
        driver.capture_provider().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window {
                id: frontmost_window.id,
            },
        }]
    );
    assert_eq!(
        driver.tree_inspector().requested_surfaces(),
        vec![Surface {
            kind: SurfaceKind::Window {
                id: frontmost_window.id,
            },
        }]
    );
}

#[tokio::test]
async fn permissions_query_returns_report() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(macos_permissions_report(
            PermissionStatus::Granted,
            PermissionStatus::Granted,
            PermissionStatus::Denied,
        )),
    );

    let result = driver
        .query(QueryRequest::PermissionsStatus, &exec_context())
        .await
        .unwrap();

    assert_eq!(
        result,
        QueryResult::Permissions(macos_permissions_report(
            PermissionStatus::Granted,
            PermissionStatus::Granted,
            PermissionStatus::Denied,
        ))
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
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let apps = driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::Running,
            },
            &exec_context(),
        )
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
    assert_eq!(
        driver.app_service().app_list_modes(),
        vec![AppListMode::Running]
    );
}

#[tokio::test]
async fn list_apps_query_bypasses_system_events_permission_probe() {
    let permissions = CountingPermissionReader::with_report(macos_permissions_report(
        PermissionStatus::Denied,
        PermissionStatus::Denied,
        PermissionStatus::Denied,
    ));
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            ..Default::default()
        },
        permissions.clone(),
    );

    let apps = driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::Running,
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
    assert_eq!(permissions.call_count(), 0);
}

#[tokio::test]
async fn list_apps_query_forwards_requested_mode_to_app_service() {
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            all_apps: Some(vec![
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
            ]),
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let apps = driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::All,
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert_eq!(
        apps,
        QueryResult::Apps(vec![
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
        ])
    );
    assert_eq!(
        driver.app_service().app_list_modes(),
        vec![AppListMode::All]
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
        bundle_id: Some("com.apple.Safari".into()),
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
                ..default_action_request()
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
    let driver = MacosDriver::new(
        StubAppService {
            windows: vec![WindowInfo {
                id: 42.into(),
                title: Some("Draft".into()),
                app_name: Some("TextEdit".into()),
                bounds: Some(Rect {
                    x: 40.0,
                    y: 60.0,
                    width: 640.0,
                    height: 480.0,
                }),
                is_focused: false,
                is_minimized: false,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::FocusWindow { id: 42.into() },
                locator: None,
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("focused window 42"));
    assert_eq!(
        outcome.target_window,
        Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 40.0,
                y: 60.0,
                width: 640.0,
                height: 480.0,
            }),
            is_focused: true,
            is_minimized: false,
        })
    );
    assert_eq!(outcome.side_effects, vec![ActionSideEffect::FocusWindow]);
    assert_eq!(
        driver.app_service().focused_windows(),
        vec![WindowId::from(42)]
    );
}

#[tokio::test]
async fn close_window_action_uses_app_target_anchor_window_without_auto_focus() {
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            windows: vec![
                WindowInfo {
                    id: 41.into(),
                    title: Some("Draft".into()),
                    app_name: Some("TextEdit".into()),
                    bounds: None,
                    is_focused: true,
                    is_minimized: false,
                },
                WindowInfo {
                    id: 42.into(),
                    title: Some("Notes".into()),
                    app_name: Some("TextEdit".into()),
                    bounds: None,
                    is_focused: false,
                    is_minimized: false,
                },
            ],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::CloseWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                focus_policy: ActionFocusPolicy::Never,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("closed window 41"));
    assert_eq!(
        driver.app_service().closed_windows(),
        vec![WindowId::from(41)]
    );
    assert!(driver.app_service().focused_apps().is_empty());
}

#[tokio::test]
async fn minimize_window_action_uses_window_title_selector_and_auto_focuses_window() {
    let driver = MacosDriver::new(
        StubAppService {
            windows: vec![WindowInfo {
                id: 42.into(),
                title: Some("Draft".into()),
                app_name: Some("TextEdit".into()),
                bounds: None,
                is_focused: false,
                is_minimized: false,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::MinimizeWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("minimized window 42"));
    assert_eq!(
        driver.app_service().minimized_windows(),
        vec![WindowId::from(42)]
    );
    assert_eq!(
        driver.app_service().focused_windows(),
        vec![WindowId::from(42)]
    );
}

#[tokio::test]
async fn maximize_window_action_uses_pid_target_anchor_window() {
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            windows: vec![WindowInfo {
                id: 43.into(),
                title: Some("Draft".into()),
                app_name: Some("TextEdit".into()),
                bounds: None,
                is_focused: false,
                is_minimized: false,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::MaximizeWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::Pid(101)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("maximized window 43"));
    assert_eq!(
        driver.app_service().maximized_windows(),
        vec![WindowId::from(43)]
    );
    assert_eq!(
        driver.app_service().focused_apps(),
        vec!["com.apple.TextEdit".to_string()]
    );
}

#[tokio::test]
async fn move_window_action_returns_post_action_geometry() {
    let driver = MacosDriver::new(
        StubAppService {
            windows: vec![WindowInfo {
                id: 42.into(),
                title: Some("Draft".into()),
                app_name: Some("TextEdit".into()),
                bounds: Some(Rect {
                    x: 40.0,
                    y: 60.0,
                    width: 640.0,
                    height: 480.0,
                }),
                is_focused: false,
                is_minimized: false,
            }],
            move_window_result: Some(Rect {
                x: 120.0,
                y: 240.0,
                width: 640.0,
                height: 480.0,
            }),
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::MoveWindow { x: 120.0, y: 240.0 },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                focus_policy: ActionFocusPolicy::Never,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        outcome.detail.as_deref(),
        Some("moved window 42 to x=120 y=240 width=640 height=480")
    );
    assert_eq!(
        outcome.target_window,
        Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 120.0,
                y: 240.0,
                width: 640.0,
                height: 480.0,
            }),
            is_focused: false,
            is_minimized: false,
        })
    );
    assert_eq!(
        outcome.side_effects,
        vec![ActionSideEffect::MoveWindow {
            bounds: Rect {
                x: 120.0,
                y: 240.0,
                width: 640.0,
                height: 480.0,
            },
        }]
    );
    assert_eq!(
        driver.app_service().moved_windows(),
        vec![(WindowId::from(42), 120.0, 240.0)]
    );
    assert!(driver.app_service().focused_windows().is_empty());
}

#[tokio::test]
async fn resize_window_action_uses_app_target_anchor_window_and_auto_focuses() {
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
            }],
            resize_window_result: Some(Rect {
                x: 120.0,
                y: 240.0,
                width: 800.0,
                height: 600.0,
            }),
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
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
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        outcome.detail.as_deref(),
        Some("resized window 42 to x=120 y=240 width=800 height=600")
    );
    assert_eq!(
        driver.app_service().resized_windows(),
        vec![(WindowId::from(42), 800.0, 600.0)]
    );
    assert_eq!(
        driver.app_service().focused_apps(),
        vec!["com.apple.TextEdit".to_string()]
    );
}

#[tokio::test]
async fn set_window_bounds_action_uses_pid_target_and_returns_post_action_geometry() {
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
                title: Some("Draft".into()),
                app_name: Some("TextEdit".into()),
                bounds: Some(Rect {
                    x: 40.0,
                    y: 60.0,
                    width: 640.0,
                    height: 480.0,
                }),
                is_focused: true,
                is_minimized: false,
            }],
            set_window_bounds_result: Some(Rect {
                x: 80.0,
                y: 120.0,
                width: 900.0,
                height: 700.0,
            }),
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
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
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        outcome.detail.as_deref(),
        Some("set window 42 bounds to x=80 y=120 width=900 height=700")
    );
    assert_eq!(
        driver.app_service().set_window_bounds_calls(),
        vec![(
            WindowId::from(42),
            Rect {
                x: 80.0,
                y: 120.0,
                width: 900.0,
                height: 700.0,
            }
        )]
    );
}

#[tokio::test]
async fn switch_app_action_uses_target_app_identity() {
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::SwitchApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("switched app"));
    assert_eq!(
        driver.app_service().focused_apps(),
        vec!["TextEdit".to_string()]
    );
}

#[tokio::test]
async fn quit_app_action_uses_pid_target_selector() {
    let driver = MacosDriver::new(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.TextEdit".into()),
                name: "TextEdit".into(),
                pid: Some(101),
                is_running: true,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::QuitApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::Pid(101)),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("quit app"));
    assert_eq!(
        driver.app_service().quit_apps(),
        vec!["TextEdit".to_string()]
    );
}

#[tokio::test]
async fn relaunch_app_action_resolves_window_title_to_app_identity() {
    let driver = MacosDriver::new(
        StubAppService {
            windows: vec![WindowInfo {
                id: 41.into(),
                title: Some("Draft".into()),
                app_name: Some("TextEdit".into()),
                bounds: None,
                is_focused: true,
                is_minimized: false,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::RelaunchApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("relaunched app"));
    assert_eq!(
        driver.app_service().relaunched_apps(),
        vec!["TextEdit".to_string()]
    );
}

#[tokio::test]
async fn hide_app_action_uses_window_index_target_selector() {
    let driver = MacosDriver::new(
        StubAppService {
            windows: vec![
                WindowInfo {
                    id: 40.into(),
                    title: Some("First".into()),
                    app_name: Some("TextEdit".into()),
                    bounds: None,
                    is_focused: false,
                    is_minimized: false,
                },
                WindowInfo {
                    id: 41.into(),
                    title: Some("Second".into()),
                    app_name: Some("Notes".into()),
                    bounds: None,
                    is_focused: true,
                    is_minimized: false,
                },
            ],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::HideApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowIndex(1)),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("hid app"));
    assert_eq!(
        driver.app_service().hidden_apps(),
        vec!["Notes".to_string()]
    );
}

#[tokio::test]
async fn unhide_app_action_uses_window_id_target_selector() {
    let driver = MacosDriver::new(
        StubAppService {
            windows: vec![WindowInfo {
                id: 42.into(),
                title: Some("Inbox".into()),
                app_name: Some("Mail".into()),
                bounds: None,
                is_focused: false,
                is_minimized: false,
            }],
            ..Default::default()
        },
        StubPermissionReader::granted(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::UnhideApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(42.into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("unhid app"));
    assert_eq!(
        driver.app_service().unhidden_apps(),
        vec!["Mail".to_string()]
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
async fn click_action_with_app_target_refreshes_anchor_window_after_auto_focus() {
    let input = StubInputSynthesizer::default();
    let app = AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    };
    let anchor_window = WindowInfo {
        id: 7.into(),
        title: Some("Calculator".into()),
        app_name: Some("Calculator".into()),
        bounds: Some(Rect {
            x: 338.0,
            y: 216.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![app.clone()],
            windows: Vec::new(),
            windows_after_focus: Some(vec![anchor_window.clone()]),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                locator: Some(Locator::Coords(Point { x: 426.0, y: 374.0 })),
                target_selector: Some(ActionTargetSelector::App("Calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        driver.app_service().focused_apps(),
        vec!["com.apple.calculator"]
    );
    assert_eq!(outcome.target_app, Some(app));
    assert_eq!(outcome.target_window, Some(anchor_window));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 426.0, y: 374.0 }),
            mode: ClickMode::Left,
        }]
    );
}

#[tokio::test]
async fn click_action_with_app_target_falls_back_to_frontmost_window_lookup() {
    let input = StubInputSynthesizer::default();
    let app = AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    };
    let anchor_window = WindowInfo {
        id: 7.into(),
        title: Some("Calculator".into()),
        app_name: Some("Calculator".into()),
        bounds: Some(Rect {
            x: 338.0,
            y: 216.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![app.clone()],
            windows: vec![anchor_window.clone()],
            filtered_windows: Some(Vec::new()),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                locator: Some(Locator::Coords(Point { x: 426.0, y: 374.0 })),
                target_selector: Some(ActionTargetSelector::App("Calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.target_app, Some(app));
    assert_eq!(outcome.target_window, Some(anchor_window));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 426.0, y: 374.0 }),
            mode: ClickMode::Left,
        }]
    );
}

#[tokio::test]
async fn click_action_with_app_target_uses_focused_window_when_window_name_is_localized() {
    let input = StubInputSynthesizer::default();
    let app = AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    };
    let localized_window = WindowInfo {
        id: 8.into(),
        title: Some("计算器".into()),
        app_name: Some("计算器".into()),
        bounds: Some(Rect {
            x: 535.0,
            y: 260.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![app.clone()],
            windows: vec![localized_window.clone()],
            filtered_windows: Some(Vec::new()),
            focus: Some(FocusInfo {
                role: "AXApplication".into(),
                label: None,
                bounds: None,
                bundle_id: Some("com.apple.calculator".into()),
                app_name: Some("计算器".into()),
            }),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                locator: Some(Locator::Coords(Point { x: 622.5, y: 417.0 })),
                target_selector: Some(ActionTargetSelector::App("Calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.target_app, Some(app));
    assert_eq!(outcome.target_window, Some(localized_window));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 622.5, y: 417.0 }),
            mode: ClickMode::Left,
        }]
    );
}

#[tokio::test]
async fn click_action_with_app_target_uses_frontmost_usable_window_when_focus_flags_lag() {
    let input = StubInputSynthesizer::default();
    let app = AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    };
    let candidate_window = WindowInfo {
        id: 9.into(),
        title: Some("计算器".into()),
        app_name: Some("计算器".into()),
        bounds: Some(Rect {
            x: 535.0,
            y: 260.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: false,
        is_minimized: false,
    };
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![app.clone()],
            windows: vec![
                WindowInfo {
                    id: 1.into(),
                    title: None,
                    app_name: Some("Codex".into()),
                    bounds: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1470.0,
                        height: 33.0,
                    }),
                    is_focused: false,
                    is_minimized: false,
                },
                candidate_window.clone(),
            ],
            filtered_windows: Some(Vec::new()),
            focus: Some(FocusInfo {
                role: "AXApplication".into(),
                label: None,
                bounds: None,
                bundle_id: Some("com.apple.calculator".into()),
                app_name: Some("计算器".into()),
            }),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                locator: Some(Locator::Coords(Point { x: 622.5, y: 417.0 })),
                target_selector: Some(ActionTargetSelector::App("Calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.target_app, Some(app));
    assert_eq!(outcome.target_window, Some(candidate_window));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 622.5, y: 417.0 }),
            mode: ClickMode::Left,
        }]
    );
}

#[tokio::test]
async fn click_action_with_app_target_ignores_non_matching_frontmost_window_when_focus_flags_lag() {
    let input = StubInputSynthesizer::default();
    let app = AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    };
    let wrong_frontmost_window = WindowInfo {
        id: 10.into(),
        title: Some("Editor".into()),
        app_name: Some("Codex".into()),
        bounds: Some(Rect {
            x: 120.0,
            y: 80.0,
            width: 920.0,
            height: 680.0,
        }),
        is_focused: true,
        is_minimized: false,
    };
    let candidate_window = WindowInfo {
        id: 11.into(),
        title: Some("计算器".into()),
        app_name: Some("计算器".into()),
        bounds: Some(Rect {
            x: 535.0,
            y: 260.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: false,
        is_minimized: false,
    };
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![app.clone()],
            windows: vec![wrong_frontmost_window, candidate_window.clone()],
            filtered_windows: Some(Vec::new()),
            focus: Some(FocusInfo {
                role: "AXApplication".into(),
                label: None,
                bounds: None,
                bundle_id: Some("com.apple.calculator".into()),
                app_name: Some("计算器".into()),
            }),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                locator: Some(Locator::Coords(Point { x: 622.5, y: 417.0 })),
                target_selector: Some(ActionTargetSelector::App("Calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.target_app, Some(app));
    assert_eq!(outcome.target_window, Some(candidate_window));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 622.5, y: 417.0 }),
            mode: ClickMode::Left,
        }]
    );
}

#[tokio::test]
async fn click_action_with_app_target_uses_frontmost_window_after_auto_focus_when_metadata_is_missing(
) {
    let input = StubInputSynthesizer::default();
    let app = AppInfo {
        bundle_id: Some("com.apple.calculator".into()),
        name: "Calculator".into(),
        pid: Some(42),
        is_running: true,
    };
    let frontmost_window = WindowInfo {
        id: 10.into(),
        title: Some("计算器".into()),
        app_name: Some("计算器".into()),
        bounds: Some(Rect {
            x: 535.0,
            y: 260.0,
            width: 230.0,
            height: 408.0,
        }),
        is_focused: false,
        is_minimized: false,
    };
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![app.clone()],
            windows: Vec::new(),
            windows_after_focus: Some(vec![
                WindowInfo {
                    id: 1.into(),
                    title: None,
                    app_name: Some("Codex".into()),
                    bounds: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1470.0,
                        height: 33.0,
                    }),
                    is_focused: false,
                    is_minimized: false,
                },
                frontmost_window.clone(),
            ]),
            filtered_windows: Some(Vec::new()),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                locator: Some(Locator::Coords(Point { x: 622.5, y: 417.0 })),
                target_selector: Some(ActionTargetSelector::App("Calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.target_app, Some(app));
    assert_eq!(outcome.target_window, Some(frontmost_window));
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 622.5, y: 417.0 }),
            mode: ClickMode::Left,
        }]
    );
    assert_eq!(driver.app_service().frontmost_window_query_count(), 1);
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
async fn click_action_focuses_window_selector_before_resolving_locator() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService {
            windows: vec![WindowInfo {
                id: WindowId::from(84),
                title: Some("Submit Sheet".into()),
                app_name: Some("TextEdit".into()),
                bounds: Some(Rect {
                    x: 10.0,
                    y: 20.0,
                    width: 300.0,
                    height: 180.0,
                }),
                is_focused: false,
                is_minimized: false,
            }],
            ..StubAppService::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::Text("submit".into())),
                target_selector: Some(ActionTargetSelector::WindowTitle("Submit Sheet".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        driver.app_service().focused_windows(),
        vec![WindowId::from(84)]
    );
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Click {
            point: Some(Point { x: 140.0, y: 50.0 }),
            mode: ClickMode::Left,
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
            capture_bounds: None,
            image_size_px: None,
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
                    clear_before: false,
                    delay_ms: None,
                    trailing_keys: Vec::new(),
                },
                locator: Some(Locator::Role {
                    role: "AXTextField".into(),
                    index: 1,
                }),
                ..default_action_request()
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
            RecordedInput::TypeText {
                text: "hello operator".into(),
                delay_ms: None,
            },
        ]
    );
}

#[tokio::test]
async fn type_action_supports_clear_delay_and_trailing_keys() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([(
                ElementId("ax-field-0".into()),
                UiElement {
                    id: ElementId("ax-field-0".into()),
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
            )]),
            root_ids: vec![ElementId("ax-field-0".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Type {
                    text: "hello operator".into(),
                    clear_before: true,
                    delay_ms: Some(25),
                    trailing_keys: vec![TypeTrailingKey::Return, TypeTrailingKey::Tab],
                },
                locator: Some(Locator::Role {
                    role: "AXTextField".into(),
                    index: 0,
                }),
                ..default_action_request()
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
            RecordedInput::Hotkey(vec!["command".into(), "a".into()]),
            RecordedInput::Press {
                key: "delete".into(),
                count: 1,
                delay_ms: Some(25),
            },
            RecordedInput::TypeText {
                text: "hello operator".into(),
                delay_ms: Some(25),
            },
            RecordedInput::Press {
                key: "return".into(),
                count: 1,
                delay_ms: Some(25),
            },
            RecordedInput::Press {
                key: "tab".into(),
                count: 1,
                delay_ms: Some(25),
            },
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
async fn move_action_resolves_text_locator_before_moving_cursor() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([(
                ElementId("ax-hover-target".into()),
                UiElement {
                    id: ElementId("ax-hover-target".into()),
                    role: "AXButton".into(),
                    label: Some("Open".into()),
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
            root_ids: vec![ElementId("ax-hover-target".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Move,
                locator: Some(Locator::Text("Open".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("moved"));
    assert_eq!(
        outcome.coordinates,
        Some(ActionCoordinates {
            point: Some(Point { x: 160.0, y: 180.0 }),
            from: None,
            to: None,
        })
    );
    assert_eq!(outcome.side_effects, vec![ActionSideEffect::MoveCursor]);
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Move {
            point: Point { x: 160.0, y: 180.0 },
        }]
    );
}

#[tokio::test]
async fn move_action_uses_window_index_selector_when_locator_is_absent() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService {
            windows: vec![
                WindowInfo {
                    id: WindowId::from(7),
                    title: Some("Ignored".into()),
                    app_name: Some("Notes".into()),
                    bounds: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 200.0,
                        height: 100.0,
                    }),
                    is_focused: false,
                    is_minimized: false,
                },
                WindowInfo {
                    id: WindowId::from(8),
                    title: Some("Target".into()),
                    app_name: Some("TextEdit".into()),
                    bounds: Some(Rect {
                        x: 300.0,
                        y: 120.0,
                        width: 400.0,
                        height: 240.0,
                    }),
                    is_focused: false,
                    is_minimized: false,
                },
            ],
            ..StubAppService::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                action: Action::Move,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowIndex(1)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        driver.app_service().focused_windows(),
        vec![WindowId::from(8)]
    );
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Move {
            point: Point { x: 500.0, y: 240.0 },
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
async fn swipe_action_resolves_between_locators_and_returns_successful_outcome() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService::default(),
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
        }),
        StubTreeInspector::with_result(InspectResult {
            elements: HashMap::from([
                (
                    ElementId("ax-start".into()),
                    UiElement {
                        id: ElementId("ax-start".into()),
                        role: "AXStaticText".into(),
                        label: Some("Swipe start".into()),
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
                    ElementId("ax-end".into()),
                    UiElement {
                        id: ElementId("ax-end".into()),
                        role: "AXButton".into(),
                        label: Some("Swipe end".into()),
                        value: None,
                        bounds: Some(Rect {
                            x: 180.0,
                            y: 20.0,
                            width: 60.0,
                            height: 20.0,
                        }),
                        enabled: Some(true),
                        children: vec![],
                        confidence: Some(1.0),
                        source: ElementSource::Native,
                    },
                ),
            ]),
            root_ids: vec![ElementId("ax-start".into()), ElementId("ax-end".into())],
        }),
        input.clone(),
    );

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::Swipe {
                    from: Locator::Text("swipe start".into()),
                    to: Locator::Role {
                        role: "AXButton".into(),
                        index: 0,
                    },
                    duration_ms: Some(240),
                    steps: Some(4.try_into().unwrap()),
                },
                locator: None,
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("swiped"));
    assert_eq!(
        outcome.coordinates,
        Some(ActionCoordinates {
            point: None,
            from: Some(Point { x: 30.0, y: 30.0 }),
            to: Some(Point { x: 210.0, y: 30.0 }),
        })
    );
    assert_eq!(
        outcome.side_effects,
        vec![ActionSideEffect::Swipe {
            duration_ms: Some(240),
            steps: Some(4.try_into().unwrap()),
        }]
    );
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Swipe {
            from: Point { x: 30.0, y: 30.0 },
            to: Point { x: 210.0, y: 30.0 },
            duration_ms: Some(240),
            steps: Some(4.try_into().unwrap()),
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
async fn hotkey_with_app_target_tolerates_anchor_window_query_failures() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.Notes".into()),
                name: "Notes".into(),
                pid: Some(202),
                is_running: true,
            }],
            list_windows_error: Some(
                "osascript failed: execution error: Error: Error: 不能获取对象。 (-1728)".into(),
            ),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                    keys: vec!["command".into(), "n".into()],
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::App("Notes".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .expect("hotkey should still succeed when anchor window lookup fails");

    assert!(outcome.success);
    assert_eq!(outcome.detail.as_deref(), Some("sent hotkey"));
    assert_eq!(
        outcome.target_app,
        Some(AppInfo {
            bundle_id: Some("com.apple.Notes".into()),
            name: "Notes".into(),
            pid: Some(202),
            is_running: true,
        })
    );
    assert_eq!(outcome.target_window, None);
    assert_eq!(
        driver.app_service().focused_apps(),
        vec!["com.apple.Notes".to_string()]
    );
    assert_eq!(
        input.calls(),
        vec![RecordedInput::Hotkey(vec!["command".into(), "n".into()])]
    );
}

#[tokio::test]
async fn type_with_app_target_tolerates_anchor_window_query_failures() {
    let input = StubInputSynthesizer::default();
    let driver = MacosDriver::with_components(
        StubAppService {
            apps: vec![AppInfo {
                bundle_id: Some("com.apple.Notes".into()),
                name: "Notes".into(),
                pid: Some(202),
                is_running: true,
            }],
            list_windows_error: Some(
                "osascript failed: execution error: Error: Error: 不能获取对象。 (-1728)".into(),
            ),
            ..Default::default()
        },
        StubPermissionReader::granted(),
        StubCaptureProvider::with_result(CaptureResult {
            artifact_id: ArtifactId("unused.png".into()),
            display_scale: None,
            capture_bounds: None,
            image_size_px: None,
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
                action: Action::Type {
                    text: "operator notes live validation".into(),
                    clear_before: false,
                    delay_ms: None,
                    trailing_keys: Vec::new(),
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::App("Notes".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
        .expect("typing should still succeed when anchor window lookup fails");

    assert!(outcome.success);
    assert_eq!(outcome.target_window, None);
    assert_eq!(
        driver.app_service().focused_apps(),
        vec!["com.apple.Notes".to_string()]
    );
    assert_eq!(
        input.calls(),
        vec![RecordedInput::TypeText {
            text: "operator notes live validation".into(),
            delay_ms: None,
        }]
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
            capture_bounds: None,
            image_size_px: None,
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
                ..default_action_request()
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
            delay_ms: None,
        }]
    );
}

#[tokio::test]
async fn health_check_requires_accessibility_for_ready_status() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(macos_permissions_report(
            PermissionStatus::Denied,
            PermissionStatus::NotDetermined,
            PermissionStatus::NotDetermined,
        )),
    );

    let health = driver.health_check().await.unwrap();

    assert!(!health.healthy);
    assert_eq!(
        health.message.as_deref(),
        Some("Accessibility permission is required for macOS automation.")
    );
    assert_eq!(
        health.permissions.status("accessibility"),
        Some(PermissionStatus::Denied)
    );
}

#[tokio::test]
async fn health_check_requires_screen_recording_for_capture_readiness() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(macos_permissions_report(
            PermissionStatus::Granted,
            PermissionStatus::Granted,
            PermissionStatus::Denied,
        )),
    );

    let health = driver.health_check().await.unwrap();

    assert!(!health.healthy);
    assert_eq!(
        health.message.as_deref(),
        Some("Screen Recording permission is required for macOS capture.")
    );
    assert_eq!(
        health.permissions.status("screen_recording"),
        Some(PermissionStatus::Denied)
    );
}

#[tokio::test]
async fn health_check_requires_system_events_for_app_and_window_queries() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(macos_permissions_report(
            PermissionStatus::Granted,
            PermissionStatus::Denied,
            PermissionStatus::Granted,
        )),
    );

    let health = driver.health_check().await.unwrap();

    assert!(!health.healthy);
    assert_eq!(
        health.message.as_deref(),
        Some("System Events access is required for macOS window queries and focus reads.")
    );
    assert_eq!(
        health.permissions.status("system_events"),
        Some(PermissionStatus::Denied)
    );
}

#[tokio::test]
async fn list_windows_query_requires_system_events_readiness() {
    let driver = MacosDriver::new(
        StubAppService::default(),
        StubPermissionReader::with_report(macos_permissions_report(
            PermissionStatus::Granted,
            PermissionStatus::Denied,
            PermissionStatus::Granted,
        )),
    );

    let error = driver
        .query(QueryRequest::ListWindows { app: None }, &exec_context())
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "permission denied: System Events access is required for macOS window queries and focus reads."
    );
}

fn exec_context() -> ExecContext {
    ExecContext {
        target: "local:macos".into(),
        session: None,
        timeout_ms: Some(500),
    }
}

fn default_action_request() -> ActionRequest {
    ActionRequest {
        action: Action::Move,
        locator: None,
        target_selector: None,
        focus_policy: ActionFocusPolicy::Auto,
        verifications: Vec::new(),
    }
}

#[derive(Default)]
struct StubAppService {
    apps: Vec<AppInfo>,
    all_apps: Option<Vec<AppInfo>>,
    windows: Vec<WindowInfo>,
    frontmost_windows: Option<Vec<WindowInfo>>,
    windows_after_focus: Option<Vec<WindowInfo>>,
    filtered_windows: Option<Vec<WindowInfo>>,
    list_windows_error: Option<String>,
    frontmost_windows_error: Option<String>,
    focus: Option<FocusInfo>,
    launched: Mutex<Vec<String>>,
    focused_apps: Mutex<Vec<String>>,
    closed_windows: Mutex<Vec<WindowId>>,
    minimized_windows: Mutex<Vec<WindowId>>,
    maximized_windows: Mutex<Vec<WindowId>>,
    moved_windows: Mutex<Vec<(WindowId, f64, f64)>>,
    resized_windows: Mutex<Vec<(WindowId, f64, f64)>>,
    set_window_bounds_calls: Mutex<Vec<(WindowId, Rect)>>,
    quit: Mutex<Vec<String>>,
    relaunched: Mutex<Vec<String>>,
    hidden: Mutex<Vec<String>>,
    unhidden: Mutex<Vec<String>>,
    focused_windows: Mutex<Vec<WindowId>>,
    last_window_filter: Mutex<Option<String>>,
    app_list_modes: Mutex<Vec<AppListMode>>,
    frontmost_window_queries: Mutex<u32>,
    move_window_result: Option<Rect>,
    resize_window_result: Option<Rect>,
    set_window_bounds_result: Option<Rect>,
}

impl StubAppService {
    fn launched_apps(&self) -> Vec<String> {
        self.launched.lock().unwrap().clone()
    }

    fn last_window_filter(&self) -> Option<String> {
        self.last_window_filter.lock().unwrap().clone()
    }

    fn focused_apps(&self) -> Vec<String> {
        self.focused_apps.lock().unwrap().clone()
    }

    fn closed_windows(&self) -> Vec<WindowId> {
        self.closed_windows.lock().unwrap().clone()
    }

    fn minimized_windows(&self) -> Vec<WindowId> {
        self.minimized_windows.lock().unwrap().clone()
    }

    fn maximized_windows(&self) -> Vec<WindowId> {
        self.maximized_windows.lock().unwrap().clone()
    }

    fn moved_windows(&self) -> Vec<(WindowId, f64, f64)> {
        self.moved_windows.lock().unwrap().clone()
    }

    fn resized_windows(&self) -> Vec<(WindowId, f64, f64)> {
        self.resized_windows.lock().unwrap().clone()
    }

    fn set_window_bounds_calls(&self) -> Vec<(WindowId, Rect)> {
        self.set_window_bounds_calls.lock().unwrap().clone()
    }

    fn quit_apps(&self) -> Vec<String> {
        self.quit.lock().unwrap().clone()
    }

    fn relaunched_apps(&self) -> Vec<String> {
        self.relaunched.lock().unwrap().clone()
    }

    fn hidden_apps(&self) -> Vec<String> {
        self.hidden.lock().unwrap().clone()
    }

    fn unhidden_apps(&self) -> Vec<String> {
        self.unhidden.lock().unwrap().clone()
    }

    fn focused_windows(&self) -> Vec<WindowId> {
        self.focused_windows.lock().unwrap().clone()
    }

    fn frontmost_window_query_count(&self) -> u32 {
        *self.frontmost_window_queries.lock().unwrap()
    }

    fn app_list_modes(&self) -> Vec<AppListMode> {
        self.app_list_modes.lock().unwrap().clone()
    }
}

impl AppService for StubAppService {
    fn list_apps(&self, mode: AppListMode) -> Result<Vec<AppInfo>, OperatorError> {
        self.app_list_modes.lock().unwrap().push(mode);
        match mode {
            AppListMode::Running => Ok(self.apps.clone()),
            AppListMode::All => Ok(self.all_apps.clone().unwrap_or_else(|| self.apps.clone())),
        }
    }

    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError> {
        *self.last_window_filter.lock().unwrap() = app.map(str::to_string);
        if let Some(message) = &self.list_windows_error {
            return Err(OperatorError::Platform(message.clone()));
        }
        if app.is_some() {
            if let Some(windows) = &self.filtered_windows {
                return Ok(windows.clone());
            }
        }
        if !self.focused_apps.lock().unwrap().is_empty() {
            if let Some(windows) = &self.windows_after_focus {
                return Ok(windows.clone());
            }
        }
        Ok(self.windows.clone())
    }

    fn list_frontmost_windows(&self) -> Result<Vec<WindowInfo>, OperatorError> {
        *self.frontmost_window_queries.lock().unwrap() += 1;
        if let Some(message) = &self.frontmost_windows_error {
            return Err(OperatorError::Platform(message.clone()));
        }
        if !self.focused_apps.lock().unwrap().is_empty() {
            if let Some(windows) = &self.windows_after_focus {
                return Ok(windows.clone());
            }
        }
        if let Some(windows) = &self.frontmost_windows {
            return Ok(windows.clone());
        }
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

    fn focus_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.focused_apps
            .lock()
            .unwrap()
            .push(bundle_id_or_name.to_string());
        Ok(())
    }

    fn close_window(&self, id: WindowId) -> Result<(), OperatorError> {
        self.closed_windows.lock().unwrap().push(id);
        Ok(())
    }

    fn minimize_window(&self, id: WindowId) -> Result<(), OperatorError> {
        self.minimized_windows.lock().unwrap().push(id);
        Ok(())
    }

    fn maximize_window(&self, id: WindowId) -> Result<(), OperatorError> {
        self.maximized_windows.lock().unwrap().push(id);
        Ok(())
    }

    fn move_window(&self, id: WindowId, x: f64, y: f64) -> Result<Rect, OperatorError> {
        self.moved_windows.lock().unwrap().push((id, x, y));
        self.move_window_result.ok_or_else(|| {
            OperatorError::Platform(format!("stub move_window result missing for window {id}"))
        })
    }

    fn resize_window(&self, id: WindowId, width: f64, height: f64) -> Result<Rect, OperatorError> {
        self.resized_windows
            .lock()
            .unwrap()
            .push((id, width, height));
        self.resize_window_result.ok_or_else(|| {
            OperatorError::Platform(format!("stub resize_window result missing for window {id}"))
        })
    }

    fn set_window_bounds(&self, id: WindowId, bounds: Rect) -> Result<Rect, OperatorError> {
        self.set_window_bounds_calls
            .lock()
            .unwrap()
            .push((id, bounds));
        self.set_window_bounds_result.ok_or_else(|| {
            OperatorError::Platform(format!(
                "stub set_window_bounds result missing for window {id}"
            ))
        })
    }

    fn quit_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.quit
            .lock()
            .unwrap()
            .push(bundle_id_or_name.to_string());
        Ok(())
    }

    fn relaunch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.relaunched
            .lock()
            .unwrap()
            .push(bundle_id_or_name.to_string());
        Ok(())
    }

    fn hide_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.hidden
            .lock()
            .unwrap()
            .push(bundle_id_or_name.to_string());
        Ok(())
    }

    fn unhide_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.unhidden
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
        Self::with_report(macos_permissions_report(
            PermissionStatus::Granted,
            PermissionStatus::Granted,
            PermissionStatus::Granted,
        ))
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

#[derive(Clone)]
struct CountingPermissionReader {
    report: PermissionsReport,
    calls: Arc<AtomicUsize>,
}

impl CountingPermissionReader {
    fn with_report(report: PermissionsReport) -> Self {
        Self {
            report,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl PermissionReader for CountingPermissionReader {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.report.clone())
    }
}

fn macos_permissions_report(
    accessibility: PermissionStatus,
    system_events: PermissionStatus,
    screen_recording: PermissionStatus,
) -> PermissionsReport {
    PermissionsReport::new([
        PermissionCheck::new("accessibility", "Accessibility", accessibility)
            .with_message("Accessibility permission is required for macOS automation."),
        PermissionCheck::new("system_events", "System Events", system_events).with_message(
            "System Events access is required for macOS window queries and focus reads.",
        ),
        PermissionCheck::new("screen_recording", "Screen Recording", screen_recording)
            .with_message("Screen Recording permission is required for macOS capture."),
    ])
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
    Move {
        point: Point,
    },
    Drag {
        from: Point,
        to: Point,
        motion: DragMotion,
    },
    Swipe {
        from: Point,
        to: Point,
        duration_ms: Option<u64>,
        steps: Option<std::num::NonZeroU32>,
    },
    Hotkey(Vec<String>),
    Press {
        key: String,
        count: u32,
        delay_ms: Option<u64>,
    },
    Scroll {
        point: Option<Point>,
        delta_x: f64,
        delta_y: f64,
    },
    TypeText {
        text: String,
        delay_ms: Option<u64>,
    },
}

fn enable_action_effects_test_mode() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        std::env::set_var(ACTION_EFFECTS_DRY_RUN_ENV, "1");
    });
}

#[derive(Clone)]
struct StubInputSynthesizer {
    calls: Arc<Mutex<Vec<RecordedInput>>>,
}

impl Default for StubInputSynthesizer {
    fn default() -> Self {
        enable_action_effects_test_mode();
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
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

    fn move_pointer(&self, point: Point) -> Result<(), OperatorError> {
        self.calls
            .lock()
            .unwrap()
            .push(RecordedInput::Move { point });
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

    fn swipe(
        &self,
        from: Point,
        to: Point,
        duration_ms: Option<u64>,
        steps: Option<std::num::NonZeroU32>,
    ) -> Result<(), OperatorError> {
        self.calls.lock().unwrap().push(RecordedInput::Swipe {
            from,
            to,
            duration_ms,
            steps,
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

    fn press(&self, key: &str, count: u32, delay_ms: Option<u64>) -> Result<(), OperatorError> {
        self.calls.lock().unwrap().push(RecordedInput::Press {
            key: key.to_string(),
            count,
            delay_ms,
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

    fn type_text(&self, text: &str, delay_ms: Option<u64>) -> Result<(), OperatorError> {
        self.calls.lock().unwrap().push(RecordedInput::TypeText {
            text: text.to_string(),
            delay_ms,
        });
        Ok(())
    }
}
