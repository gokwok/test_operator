use std::{
    num::NonZeroU32,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use hmdriver_rs::{
    CorrelatedWindow, CorrelatedWindowList, CurrentApp, MissionEntry, WindowEntry, WindowRect,
};
use operator_core::{
    Action, ActionFocusPolicy, ActionRequest, ActionSideEffect, ActionTargetSelector, ClickMode,
    DragModifier, DragMotion, DriverConfig, ExecContext, ImageSizePx, Locator, PlatformDriver,
    Point, Rect, TargetDescriptor, TargetId, TypeTrailingKey,
};
use operator_platform_harmony::{
    HarmonyHdcConfig, HarmonyHdcDriverFactory, HarmonyHdcSessionFactory, HarmonyHdcShellSession,
    HarmonyHdcUiSession,
};
use operator_runtime::PlatformDriverFactory;
use serde_json::json;

#[test]
fn harmony_driver_declares_pointer_and_keyboard_capabilities() {
    let driver = build_driver(FakeSessionFactory::default());
    let capabilities = driver.capabilities();

    assert!(capabilities.supports(&operator_core::Capability::Capture));
    assert!(capabilities.supports(&operator_core::Capability::PointerInput));
    assert!(capabilities.supports(&operator_core::Capability::KeyboardInput));
    assert!(capabilities.supports(&operator_core::Capability::AppLifecycle));
    assert!(capabilities.supports(&operator_core::Capability::WindowQuery));
    assert!(capabilities.supports(&operator_core::Capability::Permissions));
}

#[tokio::test]
async fn click_and_type_actions_resolve_locators_and_cover_first_phase_click_modes() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        recorded_actions: Arc::clone(&actions),
        text_locators: vec![("Submit".into(), Point { x: 320.0, y: 240.0 })],
        role_locators: vec![("TextField".into(), 0, Point { x: 410.0, y: 280.0 })],
        ..Default::default()
    });

    let clicked = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Right,
                },
                locator: Some(Locator::Text("Submit".into())),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("right click should succeed");
    let double_clicked = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Double,
                },
                locator: Some(Locator::Coords(Point { x: 500.0, y: 360.0 })),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("double click should succeed");
    let typed = driver
        .act(
            ActionRequest {
                action: Action::Type {
                    text: "hello harmony".into(),
                    clear_before: true,
                    delay_ms: None,
                    trailing_keys: vec![
                        TypeTrailingKey::Return,
                        TypeTrailingKey::Tab,
                        TypeTrailingKey::Escape,
                        TypeTrailingKey::Delete,
                    ],
                },
                locator: Some(Locator::Role {
                    role: "TextField".into(),
                    index: 0,
                }),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("type should succeed");

    assert_eq!(clicked.detail.as_deref(), Some("right-clicked"));
    assert_eq!(double_clicked.detail.as_deref(), Some("double-clicked"));
    assert_eq!(typed.detail.as_deref(), Some("typed"));
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::Click {
                point: Point { x: 320.0, y: 240.0 },
                mode: ClickMode::Right,
            },
            RecordedShellAction::Click {
                point: Point { x: 500.0, y: 360.0 },
                mode: ClickMode::Double,
            },
            RecordedShellAction::Click {
                point: Point { x: 410.0, y: 280.0 },
                mode: ClickMode::Left,
            },
            RecordedShellAction::PressKeys(vec![2072, 2017]),
            RecordedShellAction::PressKeys(vec![2055]),
            RecordedShellAction::InputText("hello harmony".into()),
            RecordedShellAction::PressKeys(vec![2054]),
            RecordedShellAction::PressKeys(vec![2049]),
            RecordedShellAction::PressKeys(vec![2070]),
            RecordedShellAction::PressKeys(vec![2055]),
        ]
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.list_windows_calls.load(Ordering::SeqCst), 0);
    assert_eq!(counts.current_app_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn missing_text_locator_surfaces_as_locator_miss_instead_of_protocol_error() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        ..Default::default()
    });

    let error = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::Text("Missing".into())),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect_err("missing text locator should fail");

    assert!(
        error
            .to_string()
            .contains("harmony.hdc could not resolve locator"),
        "unexpected error: {error}"
    );
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn permissions_query_reuses_existing_ui_session_after_locator_actions() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        text_locators: vec![("Submit".into(), Point { x: 320.0, y: 240.0 })],
        ..Default::default()
    });

    driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::Text("Submit".into())),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("text click should establish the ui session");

    let result = driver
        .query(
            operator_core::QueryRequest::PermissionsStatus,
            &exec_context(),
        )
        .await
        .expect("permissions query should succeed");

    let operator_core::QueryResult::Permissions(report) = result else {
        panic!("expected permissions report");
    };

    assert_eq!(
        report.status(operator_platform_harmony::HDC_UI_BRIDGE_CHECK_ID),
        Some(operator_core::PermissionStatus::Granted)
    );
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn selector_miss_keeps_ui_session_available_for_permissions_probe() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        ..Default::default()
    });

    let error = driver
        .act(
            ActionRequest {
                action: Action::Click {
                    mode: ClickMode::Left,
                },
                locator: Some(Locator::Text("Missing".into())),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect_err("missing text locator should fail");
    assert!(
        error
            .to_string()
            .contains("harmony.hdc could not resolve locator"),
        "unexpected error: {error}"
    );

    let result = driver
        .query(
            operator_core::QueryRequest::PermissionsStatus,
            &exec_context(),
        )
        .await
        .expect("permissions query should reuse the existing ui session");

    let operator_core::QueryResult::Permissions(report) = result else {
        panic!("expected permissions report");
    };

    assert_eq!(
        report.status(operator_platform_harmony::HDC_UI_BRIDGE_CHECK_ID),
        Some(operator_core::PermissionStatus::Granted)
    );
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn press_and_hotkey_actions_focus_requested_target_before_keyboard_input() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        recorded_actions: Arc::clone(&actions),
        current_app: Some(CurrentApp {
            bundle_name: "com.demo.notes".into(),
            ability_name: "EntryAbility".into(),
        }),
        windows: CorrelatedWindowList {
            windows: vec![
                CorrelatedWindow {
                    window: window(7, "Draft", 101, 40, 50, 600, 400),
                    mission: Some(mission(7, "Notes", "com.demo.notes")),
                },
                CorrelatedWindow {
                    window: window(9, "Calculator", 102, 680, 50, 320, 480),
                    mission: Some(mission(9, "Calculator", "com.demo.calculator")),
                },
            ],
            focused_window_id: Some(7),
            highlighted_window_ids: vec![7],
            total_window_count: Some(2),
        },
        ..Default::default()
    });

    let pressed = driver
        .act(
            ActionRequest {
                action: Action::Press {
                    key: "down".into(),
                    count: NonZeroU32::new(2).unwrap(),
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::App("notes".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("press should succeed");
    let hotkey = driver
        .act(
            ActionRequest {
                action: Action::Hotkey {
                    keys: vec!["command".into(), "shift".into(), "p".into()],
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("hotkey should succeed");

    assert_eq!(pressed.detail.as_deref(), Some("pressed down 2 times"));
    assert_eq!(
        pressed.target_app.as_ref().map(|app| app.name.as_str()),
        Some("Notes")
    );
    assert_eq!(
        pressed.target_window.as_ref().map(|window| window.id),
        Some(7_u64.into())
    );
    assert_eq!(hotkey.detail.as_deref(), Some("sent hotkey"));
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::Click {
                point: Point { x: 340.0, y: 250.0 },
                mode: ClickMode::Left,
            },
            RecordedShellAction::PressKeys(vec![2013]),
            RecordedShellAction::PressKeys(vec![2013]),
            RecordedShellAction::Click {
                point: Point { x: 340.0, y: 250.0 },
                mode: ClickMode::Left,
            },
            RecordedShellAction::PressKeys(vec![2076, 2047, 2032]),
        ]
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 0);
    assert_eq!(counts.list_windows_calls.load(Ordering::SeqCst), 2);
    assert_eq!(counts.current_app_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn drag_and_swipe_actions_convert_duration_to_velocity_and_warn_on_ignored_fields() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        recorded_actions: Arc::clone(&actions),
        ..Default::default()
    });

    let dragged = driver
        .act(
            ActionRequest {
                action: Action::Drag {
                    from: Locator::Coords(Point { x: 10.0, y: 20.0 }),
                    to: Locator::Coords(Point { x: 110.0, y: 20.0 }),
                    motion: DragMotion {
                        duration_ms: Some(250),
                        steps: Some(NonZeroU32::new(6).unwrap()),
                        modifiers: vec![DragModifier::Command, DragModifier::Shift],
                    },
                },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("drag should succeed");
    let swiped = driver
        .act(
            ActionRequest {
                action: Action::Swipe {
                    from: Locator::Coords(Point { x: 12.0, y: 24.0 }),
                    to: Locator::Coords(Point { x: 212.0, y: 24.0 }),
                    duration_ms: Some(500),
                    steps: Some(NonZeroU32::new(3).unwrap()),
                },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("swipe should succeed");

    assert_eq!(
        dragged.warnings,
        vec![
            "harmony.hdc ignores drag step counts in the first phase",
            "harmony.hdc ignores drag modifiers in the first phase",
        ]
    );
    assert_eq!(
        swiped.warnings,
        vec!["harmony.hdc ignores swipe step counts in the first phase"]
    );
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::Drag {
                from: Point { x: 10.0, y: 20.0 },
                to: Point { x: 110.0, y: 20.0 },
                speed: Some(400),
            },
            RecordedShellAction::Swipe {
                from: Point { x: 12.0, y: 24.0 },
                to: Point { x: 212.0, y: 24.0 },
                speed: Some(400),
            },
        ]
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn move_and_scroll_actions_use_cursor_move_and_swipe_fallbacks() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let driver = build_driver(FakeSessionFactory {
        recorded_actions: Arc::clone(&actions),
        current_app: Some(CurrentApp {
            bundle_name: "com.demo.notes".into(),
            ability_name: "EntryAbility".into(),
        }),
        windows: CorrelatedWindowList {
            windows: vec![CorrelatedWindow {
                window: window(7, "Draft", 101, 40, 50, 600, 400),
                mission: Some(mission(7, "Notes", "com.demo.notes")),
            }],
            focused_window_id: Some(7),
            highlighted_window_ids: vec![7],
            total_window_count: Some(1),
        },
        ..Default::default()
    });

    let moved = driver
        .act(
            ActionRequest {
                action: Action::Move,
                locator: Some(Locator::Coords(Point { x: 640.0, y: 360.0 })),
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("move should succeed");
    let scrolled = driver
        .act(
            ActionRequest {
                action: Action::Scroll {
                    delta_x: 0.0,
                    delta_y: -120.0,
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("scroll should succeed");

    assert_eq!(moved.detail.as_deref(), Some("moved"));
    assert_eq!(
        moved.coordinates.as_ref().and_then(|coords| coords.point),
        Some(Point { x: 640.0, y: 360.0 })
    );
    assert_eq!(moved.side_effects, vec![ActionSideEffect::MoveCursor]);
    assert_eq!(scrolled.detail.as_deref(), Some("scrolled"));
    assert_eq!(
        scrolled.side_effects,
        vec![ActionSideEffect::Scroll {
            delta_x: 0.0,
            delta_y: -120.0,
        }]
    );
    assert_eq!(
        scrolled.warnings,
        vec!["harmony.hdc approximates scroll with swipe input"]
    );
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::Move(Point { x: 640.0, y: 360.0 }),
            RecordedShellAction::Swipe {
                from: Point { x: 340.0, y: 314.8 },
                to: Point { x: 340.0, y: 185.2 },
                speed: Some(2_000),
            },
        ]
    );
}

fn build_driver(factory: FakeSessionFactory) -> Arc<dyn PlatformDriver> {
    HarmonyHdcDriverFactory::new_with_session_factory(Arc::new(factory))
        .build(&TargetDescriptor {
            id: TargetId("harmony-pc".into()),
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            driver_config: DriverConfig::from([("addr".into(), json!("192.168.8.43:35319"))]),
        })
        .expect("factory should build harmony driver")
}

fn exec_context() -> ExecContext {
    ExecContext {
        target: "harmony-pc".into(),
        session: None,
        timeout_ms: Some(1_000),
    }
}

fn window(
    window_id: u32,
    name: &str,
    pid: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> WindowEntry {
    WindowEntry {
        name: name.into(),
        display_id: 0,
        pid,
        window_id,
        window_type: 0,
        mode: 0,
        flag: 0,
        z_order: 0,
        orientation: 0,
        rect: WindowRect {
            x,
            y,
            width,
            height,
        },
    }
}

fn mission(mission_id: u32, app_name: &str, bundle_name: &str) -> MissionEntry {
    MissionEntry {
        mission_id,
        mission_name: app_name.into(),
        locked_state: 0,
        mission_affinity: bundle_name.into(),
        ability_record_id: None,
        app_name: Some(app_name.into()),
        main_name: Some("EntryAbility".into()),
        bundle_name: Some(bundle_name.into()),
        ability_type: Some("page".into()),
        state: Some("FOREGROUND".into()),
        app_state: Some("RUNNING".into()),
        ready: Some(true),
        window_attached: Some(true),
        launcher: Some(false),
        is_keep_alive: Some(false),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RecordedShellAction {
    Move(Point),
    Click {
        point: Point,
        mode: ClickMode,
    },
    InputText(String),
    PressKeys(Vec<u32>),
    Drag {
        from: Point,
        to: Point,
        speed: Option<u32>,
    },
    Swipe {
        from: Point,
        to: Point,
        speed: Option<u32>,
    },
}

#[derive(Default)]
struct CallCounts {
    shell_connects: AtomicUsize,
    ui_connects: AtomicUsize,
    list_windows_calls: AtomicUsize,
    current_app_calls: AtomicUsize,
}

#[derive(Clone)]
struct FakeSessionFactory {
    counts: Arc<CallCounts>,
    recorded_actions: Arc<Mutex<Vec<RecordedShellAction>>>,
    current_app: Option<CurrentApp>,
    windows: CorrelatedWindowList,
    text_locators: Vec<(String, Point)>,
    role_locators: Vec<(String, usize, Point)>,
}

impl Default for FakeSessionFactory {
    fn default() -> Self {
        Self {
            counts: Arc::new(CallCounts::default()),
            recorded_actions: Arc::new(Mutex::new(Vec::new())),
            current_app: None,
            windows: CorrelatedWindowList {
                windows: Vec::new(),
                focused_window_id: None,
                highlighted_window_ids: Vec::new(),
                total_window_count: Some(0),
            },
            text_locators: Vec::new(),
            role_locators: Vec::new(),
        }
    }
}

impl HarmonyHdcSessionFactory for FakeSessionFactory {
    fn connect_shell(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcShellSession>, operator_core::OperatorError> {
        self.counts.shell_connects.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeShellSession {
            counts: Arc::clone(&self.counts),
            recorded_actions: Arc::clone(&self.recorded_actions),
            current_app: self.current_app.clone(),
            windows: self.windows.clone(),
        }))
    }

    fn connect_ui(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, operator_core::OperatorError> {
        self.counts.ui_connects.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeUiSession {
            text_locators: self.text_locators.clone(),
            role_locators: self.role_locators.clone(),
        }))
    }
}

struct FakeShellSession {
    counts: Arc<CallCounts>,
    recorded_actions: Arc<Mutex<Vec<RecordedShellAction>>>,
    current_app: Option<CurrentApp>,
    windows: CorrelatedWindowList,
}

impl HarmonyHdcShellSession for FakeShellSession {
    fn exec_checked(&mut self, _command: &str) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn screenshot_probe(&mut self) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn capture_screenshot(&mut self, _path: &Path) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn display_size(&mut self) -> Result<ImageSizePx, operator_core::OperatorError> {
        Ok(ImageSizePx {
            width: 1920,
            height: 1080,
        })
    }

    fn focused_window_bounds(&mut self) -> Result<Option<Rect>, operator_core::OperatorError> {
        Ok(None)
    }

    fn list_apps(&mut self) -> Result<Vec<String>, operator_core::OperatorError> {
        Ok(Vec::new())
    }

    fn list_app_labels(
        &mut self,
    ) -> Result<Vec<hmdriver_rs::AppLabelInfo>, operator_core::OperatorError> {
        Ok(Vec::new())
    }

    fn filter_desktop_bundles(
        &mut self,
        bundles: &[String],
    ) -> Result<Vec<String>, operator_core::OperatorError> {
        Ok(bundles.to_vec())
    }

    fn current_app(&mut self) -> Result<Option<CurrentApp>, operator_core::OperatorError> {
        self.counts.current_app_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.current_app.clone())
    }

    fn list_windows_with_missions(
        &mut self,
    ) -> Result<CorrelatedWindowList, operator_core::OperatorError> {
        self.counts
            .list_windows_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.windows.clone())
    }

    fn click(&mut self, point: Point, mode: ClickMode) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::Click { point, mode });
        Ok(())
    }

    fn move_cursor(&mut self, point: Point) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::Move(point));
        Ok(())
    }

    fn input_text(&mut self, text: &str) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::InputText(text.into()));
        Ok(())
    }

    fn press_keys(&mut self, keys: &[u32]) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::PressKeys(keys.to_vec()));
        Ok(())
    }

    fn start_app(
        &mut self,
        _bundle: &str,
        _ability: Option<&str>,
    ) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn stop_app(&mut self, _bundle: &str) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn drag(
        &mut self,
        from: Point,
        to: Point,
        speed: Option<u32>,
    ) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::Drag { from, to, speed });
        Ok(())
    }

    fn swipe(
        &mut self,
        from: Point,
        to: Point,
        speed: Option<u32>,
    ) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::Swipe { from, to, speed });
        Ok(())
    }
}

struct FakeUiSession {
    text_locators: Vec<(String, Point)>,
    role_locators: Vec<(String, usize, Point)>,
}

impl HarmonyHdcUiSession for FakeUiSession {
    fn check_ready(&self) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn resolve_locator(
        &mut self,
        locator: &Locator,
    ) -> Result<Option<Point>, operator_core::OperatorError> {
        match locator {
            Locator::Coords(point) => Ok(Some(*point)),
            Locator::Text(text) => Ok(self
                .text_locators
                .iter()
                .find_map(|(candidate, point)| (candidate == text).then_some(*point))),
            Locator::Role { role, index } => Ok(self.role_locators.iter().find_map(
                |(candidate_role, candidate_index, point)| {
                    (candidate_role == role && candidate_index == index).then_some(*point)
                },
            )),
            Locator::SnapshotElement { .. }
            | Locator::SnapshotPixelCoords { .. }
            | Locator::SnapshotCoords { .. }
            | Locator::SnapshotNormalizedCoords { .. } => Ok(None),
        }
    }
}
