use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use hmdriver_rs::{
    AppLabelInfo, CorrelatedWindow, CorrelatedWindowList, CurrentApp, MissionEntry, WindowEntry,
    WindowRect,
};
use operator_core::{
    Action, ActionFocusPolicy, ActionRequest, ActionSideEffect, ActionTargetSelector, ClickMode,
    DriverConfig, ExecContext, ImageSizePx, PlatformDriver, Point, Rect, TargetDescriptor,
    TargetId,
};
use operator_platform_harmony::{
    HarmonyActiveWindow, HarmonyHdcConfig, HarmonyHdcDriverFactory, HarmonyHdcSessionFactory,
    HarmonyHdcShellSession, HarmonyHdcUiSession,
};
use operator_runtime::PlatformDriverFactory;
use serde_json::json;

#[tokio::test]
async fn launch_app_resolves_installed_bundle_ids_and_running_app_names() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        recorded_actions: Arc::clone(&actions),
        installed_apps: vec!["com.demo.camera".into(), "com.demo.notes".into()],
        app_labels: Vec::new(),
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

    let launched_by_bundle = driver
        .act(
            ActionRequest {
                action: Action::LaunchApp {
                    bundle_id_or_name: "COM.DEMO.CAMERA".into(),
                },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("launch-app should accept installed bundle ids case-insensitively");
    let launched_by_name = driver
        .act(
            ActionRequest {
                action: Action::LaunchApp {
                    bundle_id_or_name: "Notes".into(),
                },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("launch-app should resolve running app names to bundle ids");

    assert_eq!(
        launched_by_bundle.detail.as_deref(),
        Some("launched com.demo.camera")
    );
    assert_eq!(
        launched_by_name.detail.as_deref(),
        Some("launched com.demo.notes")
    );
    assert_eq!(
        launched_by_bundle.side_effects,
        vec![ActionSideEffect::LaunchApp]
    );
    assert_eq!(
        launched_by_name.side_effects,
        vec![ActionSideEffect::LaunchApp]
    );
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::StartApp {
                bundle: "com.demo.camera".into(),
                ability: None,
            },
            RecordedShellAction::StartApp {
                bundle: "com.demo.notes".into(),
                ability: None,
            },
        ]
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.current_app_calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.list_windows_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn launch_app_resolves_installed_display_names_from_catalog() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let driver = build_driver(FakeSessionFactory {
        recorded_actions: Arc::clone(&actions),
        installed_apps: vec!["com.demo.notes".into()],
        app_labels: vec![app_label("com.demo.notes", "备忘录")],
        ..Default::default()
    });

    let launched = driver
        .act(
            ActionRequest {
                action: Action::LaunchApp {
                    bundle_id_or_name: "备忘录".into(),
                },
                locator: None,
                target_selector: None,
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("launch-app should resolve installed display names from the Harmony catalog");

    assert_eq!(launched.detail.as_deref(), Some("launched com.demo.notes"));
    assert_eq!(launched.side_effects, vec![ActionSideEffect::LaunchApp]);
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![RecordedShellAction::StartApp {
            bundle: "com.demo.notes".into(),
            ability: None,
        }]
    );
}

#[tokio::test]
async fn switch_quit_and_relaunch_actions_use_resolved_target_bundles() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        recorded_actions: Arc::clone(&actions),
        app_labels: vec![
            app_label("com.demo.notes", "备忘录"),
            app_label("com.demo.calculator", "计算器"),
        ],
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
        installed_apps: vec!["com.demo.notes".into(), "com.demo.calculator".into()],
        ..Default::default()
    });

    let switched = driver
        .act(
            ActionRequest {
                action: Action::SwitchApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("计算器".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("switch-app should start the resolved target bundle");
    let quit = driver
        .act(
            ActionRequest {
                action: Action::QuitApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::Pid(101)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("quit-app should stop the resolved target bundle");
    let relaunched = driver
        .act(
            ActionRequest {
                action: Action::RelaunchApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("relaunch-app should stop and restart the resolved target bundle");

    assert_eq!(switched.detail.as_deref(), Some("switched app"));
    assert_eq!(quit.detail.as_deref(), Some("quit app"));
    assert_eq!(relaunched.detail.as_deref(), Some("relaunched app"));
    assert_eq!(switched.side_effects, vec![ActionSideEffect::SwitchApp]);
    assert_eq!(quit.side_effects, vec![ActionSideEffect::QuitApp]);
    assert_eq!(relaunched.side_effects, vec![ActionSideEffect::RelaunchApp]);
    assert_eq!(
        switched
            .target_app
            .as_ref()
            .and_then(|app| app.bundle_id.as_deref()),
        Some("com.demo.calculator")
    );
    assert_eq!(
        quit.target_app
            .as_ref()
            .and_then(|app| app.bundle_id.as_deref()),
        Some("com.demo.notes")
    );
    assert_eq!(
        relaunched
            .target_window
            .as_ref()
            .and_then(|window| window.title.as_deref()),
        Some("Draft")
    );
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::StartApp {
                bundle: "com.demo.calculator".into(),
                ability: None,
            },
            RecordedShellAction::StopApp("com.demo.notes".into()),
            RecordedShellAction::StopApp("com.demo.notes".into()),
            RecordedShellAction::StartApp {
                bundle: "com.demo.notes".into(),
                ability: None,
            },
        ]
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.current_app_calls.load(Ordering::SeqCst), 3);
    assert_eq!(counts.list_windows_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn hide_and_unhide_actions_click_matching_dock_icons() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let driver = build_driver(FakeSessionFactory {
        recorded_actions: Arc::clone(&actions),
        app_labels: vec![app_label("com.demo.notes", "备忘录")],
        current_app: Some(CurrentApp {
            bundle_name: "com.demo.notes".into(),
            ability_name: "EntryAbility".into(),
        }),
        active_window: Some(HarmonyActiveWindow {
            bundle_id: "com.demo.notes".into(),
            title: Some("Draft".into()),
            bounds: Some(Rect {
                x: 40.0,
                y: 50.0,
                width: 600.0,
                height: 400.0,
            }),
            is_focused: true,
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
        dock_icons: vec![(
            "备忘录".into(),
            Point {
                x: 960.0,
                y: 1040.0,
            },
        )],
        ..Default::default()
    });

    let hidden = driver
        .act(
            ActionRequest {
                action: Action::HideApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("备忘录".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("hide-app should use the dock fallback");
    let unhidden = driver
        .act(
            ActionRequest {
                action: Action::UnhideApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("unhide-app should use the same dock fallback");

    assert_eq!(hidden.detail.as_deref(), Some("hid app"));
    assert_eq!(unhidden.detail.as_deref(), Some("unhid app"));
    assert_eq!(
        hidden.side_effects,
        vec![
            ActionSideEffect::Click {
                mode: ClickMode::Left,
            },
            ActionSideEffect::HideApp,
        ]
    );
    assert_eq!(
        unhidden.side_effects,
        vec![
            ActionSideEffect::Click {
                mode: ClickMode::Left,
            },
            ActionSideEffect::UnhideApp,
        ]
    );
    assert_eq!(
        hidden.warnings,
        vec!["harmony.hdc approximates app visibility by clicking the dock icon"]
    );
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![
            RecordedShellAction::Click {
                point: Point {
                    x: 960.0,
                    y: 1040.0
                },
                mode: ClickMode::Left,
            },
            RecordedShellAction::Click {
                point: Point {
                    x: 960.0,
                    y: 1040.0
                },
                mode: ClickMode::Left,
            },
        ]
    );
}

#[tokio::test]
async fn hide_app_requires_frontmost_visible_target() {
    let driver = build_driver(FakeSessionFactory {
        app_labels: vec![
            app_label("com.demo.notes", "备忘录"),
            app_label("com.ohos.sceneboard", "桌面"),
        ],
        windows: CorrelatedWindowList {
            windows: vec![CorrelatedWindow {
                window: window(7, "Draft", 101, 40, 50, 600, 400),
                mission: Some(mission(7, "Notes", "com.demo.notes")),
            }],
            focused_window_id: None,
            highlighted_window_ids: Vec::new(),
            total_window_count: Some(1),
        },
        active_window: Some(HarmonyActiveWindow {
            bundle_id: "com.ohos.sceneboard".into(),
            title: Some("桌面".into()),
            bounds: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            }),
            is_focused: true,
        }),
        dock_icons: vec![(
            "备忘录".into(),
            Point {
                x: 960.0,
                y: 1040.0,
            },
        )],
        ..Default::default()
    });

    let error = driver
        .act(
            ActionRequest {
                action: Action::HideApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("备忘录".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect_err("hide-app should reject non-frontmost targets");

    assert!(
        error.to_string().contains("frontmost and visible"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn unhide_app_resolves_installed_targets_without_running_windows() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let driver = build_driver(FakeSessionFactory {
        recorded_actions: Arc::clone(&actions),
        installed_apps: vec!["com.demo.notes".into()],
        app_labels: vec![app_label("com.demo.notes", "备忘录")],
        dock_icons: vec![(
            "备忘录".into(),
            Point {
                x: 960.0,
                y: 1040.0,
            },
        )],
        ..Default::default()
    });

    let outcome = driver
        .act(
            ActionRequest {
                action: Action::UnhideApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("备忘录".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
        .expect("unhide-app should resolve installed app labels");

    assert_eq!(outcome.detail.as_deref(), Some("unhid app"));
    assert_eq!(
        outcome
            .target_app
            .as_ref()
            .and_then(|app| app.bundle_id.as_deref()),
        Some("com.demo.notes")
    );
    assert_eq!(
        actions.lock().unwrap().clone(),
        vec![RecordedShellAction::Click {
            point: Point {
                x: 960.0,
                y: 1040.0
            },
            mode: ClickMode::Left,
        }]
    );
}

fn build_driver(factory: FakeSessionFactory) -> Arc<dyn PlatformDriver> {
    let artifacts_dir = test_artifacts_dir();
    HarmonyHdcDriverFactory::new_with_session_factory_and_artifacts_dir(
        Arc::new(factory),
        &artifacts_dir,
    )
    .build(&TargetDescriptor {
        id: TargetId("harmony-pc".into()),
        platform: "harmony".into(),
        driver: "harmony.hdc".into(),
        driver_config: DriverConfig::from([("addr".into(), json!("192.168.8.43:35319"))]),
    })
    .expect("factory should build harmony driver")
}

fn test_artifacts_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

    let dir = std::env::temp_dir().join(format!(
        "operator-harmony-action-app-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("test artifacts dir should be created");
    dir.join("artifacts")
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
    Click {
        point: Point,
        mode: ClickMode,
    },
    StartApp {
        bundle: String,
        ability: Option<String>,
    },
    StopApp(String),
}

#[derive(Default)]
struct CallCounts {
    shell_connects: AtomicUsize,
    current_app_calls: AtomicUsize,
    list_windows_calls: AtomicUsize,
}

#[derive(Clone)]
struct FakeSessionFactory {
    counts: Arc<CallCounts>,
    recorded_actions: Arc<Mutex<Vec<RecordedShellAction>>>,
    installed_apps: Vec<String>,
    app_labels: Vec<AppLabelInfo>,
    current_app: Option<CurrentApp>,
    windows: CorrelatedWindowList,
    active_window: Option<HarmonyActiveWindow>,
    dock_icons: Vec<(String, Point)>,
}

impl Default for FakeSessionFactory {
    fn default() -> Self {
        Self {
            counts: Arc::new(CallCounts::default()),
            recorded_actions: Arc::new(Mutex::new(Vec::new())),
            installed_apps: Vec::new(),
            app_labels: Vec::new(),
            current_app: None,
            windows: CorrelatedWindowList {
                windows: Vec::new(),
                focused_window_id: None,
                highlighted_window_ids: Vec::new(),
                total_window_count: Some(0),
            },
            active_window: None,
            dock_icons: Vec::new(),
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
            installed_apps: self.installed_apps.clone(),
            app_labels: self.app_labels.clone(),
            current_app: self.current_app.clone(),
            windows: self.windows.clone(),
        }))
    }

    fn connect_ui(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, operator_core::OperatorError> {
        Ok(Box::new(FakeUiSession {
            active_window: self.active_window.clone(),
            dock_icons: self.dock_icons.clone(),
        }))
    }
}

struct FakeShellSession {
    counts: Arc<CallCounts>,
    recorded_actions: Arc<Mutex<Vec<RecordedShellAction>>>,
    installed_apps: Vec<String>,
    app_labels: Vec<AppLabelInfo>,
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
        Ok(self.installed_apps.clone())
    }

    fn list_app_labels(
        &mut self,
    ) -> Result<Vec<hmdriver_rs::AppLabelInfo>, operator_core::OperatorError> {
        Ok(self.app_labels.clone())
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

    fn click(
        &mut self,
        point: Point,
        mode: operator_core::ClickMode,
    ) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::Click { point, mode });
        Ok(())
    }

    fn input_text(&mut self, _text: &str) -> Result<(), operator_core::OperatorError> {
        unreachable!("app lifecycle tests should not type")
    }

    fn press_keys(&mut self, _keys: &[u32]) -> Result<(), operator_core::OperatorError> {
        unreachable!("app lifecycle tests should not send keys")
    }

    fn start_app(
        &mut self,
        bundle: &str,
        ability: Option<&str>,
    ) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::StartApp {
                bundle: bundle.into(),
                ability: ability.map(ToOwned::to_owned),
            });
        Ok(())
    }

    fn stop_app(&mut self, bundle: &str) -> Result<(), operator_core::OperatorError> {
        self.recorded_actions
            .lock()
            .unwrap()
            .push(RecordedShellAction::StopApp(bundle.into()));
        Ok(())
    }

    fn drag(
        &mut self,
        _from: Point,
        _to: Point,
        _speed: Option<u32>,
    ) -> Result<(), operator_core::OperatorError> {
        unreachable!("app lifecycle tests should not drag")
    }

    fn swipe(
        &mut self,
        _from: Point,
        _to: Point,
        _speed: Option<u32>,
    ) -> Result<(), operator_core::OperatorError> {
        unreachable!("app lifecycle tests should not swipe")
    }
}

struct FakeUiSession {
    active_window: Option<HarmonyActiveWindow>,
    dock_icons: Vec<(String, Point)>,
}

fn app_label(bundle_name: &str, label: &str) -> AppLabelInfo {
    AppLabelInfo {
        bundle_name: bundle_name.into(),
        label: label.into(),
    }
}

impl HarmonyHdcUiSession for FakeUiSession {
    fn check_ready(&self) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn resolve_locator(
        &mut self,
        _locator: &operator_core::Locator,
    ) -> Result<Option<Point>, operator_core::OperatorError> {
        Ok(None)
    }

    fn active_window(
        &mut self,
    ) -> Result<Option<HarmonyActiveWindow>, operator_core::OperatorError> {
        Ok(self.active_window.clone())
    }

    fn dock_app_icon_point(
        &mut self,
        labels: &[String],
    ) -> Result<Option<Point>, operator_core::OperatorError> {
        Ok(self.dock_icons.iter().find_map(|(label, point)| {
            labels
                .iter()
                .any(|candidate| candidate == label)
                .then_some(*point)
        }))
    }
}
