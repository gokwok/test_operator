use std::path::Path;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::{
    thread,
    time::{Duration, Instant},
};

use hmdriver_rs::{
    AppLabelInfo, CorrelatedWindow, CorrelatedWindowList, CurrentApp, MissionEntry, WindowEntry,
    WindowRect,
};
use operator_core::{
    AppListFilter, AppListMode, DriverConfig, ExecContext, ImageSizePx, PermissionStatus,
    PlatformDriver, QueryRequest, QueryResult, Rect, TargetDescriptor, TargetId,
};
use operator_platform_harmony::{
    HarmonyHdcConfig, HarmonyHdcDriverFactory, HarmonyHdcSessionFactory, HarmonyHdcShellSession,
    HarmonyHdcUiSession, HDC_CAPTURE_CHECK_ID, HDC_CONNECT_CHECK_ID, HDC_SHELL_CHECK_ID,
    HDC_UI_BRIDGE_CHECK_ID,
};
use operator_runtime::PlatformDriverFactory;
use serde_json::json;

#[test]
fn harmony_driver_declares_query_capabilities() {
    let driver = build_driver(FakeSessionFactory::default());
    let capabilities = driver.capabilities();

    assert!(capabilities.supports(&operator_core::Capability::Capture));
    assert!(capabilities.supports(&operator_core::Capability::PointerInput));
    assert!(capabilities.supports(&operator_core::Capability::KeyboardInput));
    assert!(capabilities.supports(&operator_core::Capability::AppLifecycle));
    assert!(capabilities.supports(&operator_core::Capability::WindowQuery));
    assert!(capabilities.supports(&operator_core::Capability::Permissions));
    assert!(!capabilities.supports(&operator_core::Capability::InspectTree));
}

#[tokio::test]
async fn permissions_query_returns_driver_scoped_checks() {
    let driver = build_driver(FakeSessionFactory {
        ui_connect: ProbeOutcome::Err("ui bridge unavailable"),
        ..Default::default()
    });

    let result = driver
        .query(QueryRequest::PermissionsStatus, &exec_context())
        .await
        .expect("permissions query should succeed");

    let QueryResult::Permissions(report) = result else {
        panic!("expected permissions result");
    };

    assert_eq!(
        report.status(HDC_CONNECT_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(
        report.status(HDC_SHELL_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(
        report.status(HDC_CAPTURE_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(
        report.status(HDC_UI_BRIDGE_CHECK_ID),
        Some(PermissionStatus::Denied)
    );
}

#[tokio::test]
async fn permissions_query_times_out_slow_shell_connect_without_waiting_for_full_probe() {
    let driver = build_driver_with_config(
        FakeSessionFactory {
            shell_connect: ProbeOutcome::Sleep(Duration::from_millis(500)),
            ..Default::default()
        },
        DriverConfig::from([
            ("addr".into(), json!("192.168.8.43:35319")),
            ("timeout_ms".into(), json!(50_u64)),
        ]),
    );

    let started = Instant::now();
    let result = driver
        .query(QueryRequest::PermissionsStatus, &exec_context())
        .await
        .expect("permissions query should return a report");
    let elapsed = started.elapsed();

    let QueryResult::Permissions(report) = result else {
        panic!("expected permissions result");
    };

    assert!(elapsed < Duration::from_millis(300));
    assert_eq!(
        report.status(HDC_CONNECT_CHECK_ID),
        Some(PermissionStatus::Denied)
    );
    assert_eq!(
        report.status(HDC_SHELL_CHECK_ID),
        Some(PermissionStatus::NotDetermined)
    );
    assert_eq!(
        report.status(HDC_CAPTURE_CHECK_ID),
        Some(PermissionStatus::NotDetermined)
    );
    assert_eq!(
        report.status(HDC_UI_BRIDGE_CHECK_ID),
        Some(PermissionStatus::NotDetermined)
    );
}

#[tokio::test]
async fn running_app_list_uses_window_backed_inventory_and_reuses_shell_session() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        apps: vec![
            "com.demo.notes".into(),
            "com.demo.calculator".into(),
            "com.demo.notes".into(),
        ],
        labels: vec![
            app_label("com.demo.notes", "备忘录"),
            app_label("com.demo.calculator", "计算器"),
        ],
        desktop_visible_bundles: vec!["com.demo.notes".into(), "com.demo.calculator".into()],
        windows: CorrelatedWindowList {
            windows: vec![
                CorrelatedWindow {
                    window: window(7, "Draft.txt", 101, 40, 50, 600, 400),
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

    let apps = driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::Running,
                filter: AppListFilter::default(),
            },
            &exec_context(),
        )
        .await
        .expect("list apps should succeed");
    let windows = driver
        .query(
            QueryRequest::ListWindows {
                app: Some("notes".into()),
            },
            &exec_context(),
        )
        .await
        .expect("list windows should succeed");

    assert_eq!(
        apps,
        QueryResult::Apps(vec![
            operator_core::AppInfo {
                bundle_id: Some("com.demo.notes".into()),
                name: "备忘录".into(),
                pid: Some(101),
                is_running: true,
            },
            operator_core::AppInfo {
                bundle_id: Some("com.demo.calculator".into()),
                name: "计算器".into(),
                pid: Some(102),
                is_running: true,
            },
        ])
    );
    assert_eq!(
        windows,
        QueryResult::Windows(vec![operator_core::WindowInfo {
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
        }])
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.list_apps_calls.load(Ordering::SeqCst), 0);
    assert_eq!(counts.list_app_labels_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        counts.filter_desktop_bundles_calls.load(Ordering::SeqCst),
        0
    );
    assert_eq!(counts.current_app_calls.load(Ordering::SeqCst), 0);
    assert_eq!(counts.list_windows_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn all_app_list_merges_installed_bundles_and_supports_filters() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        apps: vec![
            "com.demo.mail".into(),
            "com.demo.notes".into(),
            "com.demo.mail".into(),
            "com.demo.storage.service".into(),
            "com.demo.quick.widget".into(),
        ],
        labels: vec![
            app_label("com.demo.mail", "邮件"),
            app_label("com.demo.notes", "备忘录"),
            app_label("com.demo.browser", "浏览器"),
            app_label("com.demo.storage.service", "Storage Service"),
            app_label("com.demo.quick.widget", "桌面控件"),
        ],
        desktop_visible_bundles: vec!["com.demo.mail".into(), "com.demo.notes".into()],
        windows: CorrelatedWindowList {
            windows: vec![
                CorrelatedWindow {
                    window: window(7, "Draft.txt", 101, 40, 50, 600, 400),
                    mission: Some(mission(7, "Notes", "com.demo.notes")),
                },
                CorrelatedWindow {
                    window: window(12, "Browser", 103, 680, 50, 320, 480),
                    mission: Some(mission(12, "Browser", "com.demo.browser")),
                },
            ],
            focused_window_id: Some(7),
            highlighted_window_ids: vec![7],
            total_window_count: Some(2),
        },
        ..Default::default()
    });

    let apps = driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::All,
                filter: AppListFilter::default(),
            },
            &exec_context(),
        )
        .await
        .expect("list apps --all should succeed");
    let filtered = driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::All,
                filter: AppListFilter {
                    name: None,
                    bundle: Some("com.demo.mail".into()),
                },
            },
            &exec_context(),
        )
        .await
        .expect("list apps --all --bundle should succeed");

    assert_eq!(
        apps,
        QueryResult::Apps(vec![
            operator_core::AppInfo {
                bundle_id: Some("com.demo.notes".into()),
                name: "备忘录".into(),
                pid: Some(101),
                is_running: true,
            },
            operator_core::AppInfo {
                bundle_id: Some("com.demo.browser".into()),
                name: "浏览器".into(),
                pid: Some(103),
                is_running: true,
            },
            operator_core::AppInfo {
                bundle_id: Some("com.demo.mail".into()),
                name: "邮件".into(),
                pid: None,
                is_running: false,
            },
        ])
    );
    assert_eq!(
        filtered,
        QueryResult::Apps(vec![operator_core::AppInfo {
            bundle_id: Some("com.demo.mail".into()),
            name: "邮件".into(),
            pid: None,
            is_running: false,
        }])
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.list_apps_calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.list_app_labels_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        counts.filter_desktop_bundles_calls.load(Ordering::SeqCst),
        2
    );
    assert_eq!(counts.current_app_calls.load(Ordering::SeqCst), 0);
    assert_eq!(counts.list_windows_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn capabilities_query_returns_declared_capability_set() {
    let driver = build_driver(FakeSessionFactory::default());

    let result = driver
        .query(QueryRequest::Capabilities, &exec_context())
        .await
        .expect("capabilities query should succeed");

    let QueryResult::Capabilities(capabilities) = result else {
        panic!("expected capabilities result");
    };

    assert!(capabilities.supports(&operator_core::Capability::Capture));
    assert!(capabilities.supports(&operator_core::Capability::AppLifecycle));
    assert!(capabilities.supports(&operator_core::Capability::WindowQuery));
    assert!(capabilities.supports(&operator_core::Capability::Permissions));
}

fn build_driver(factory: FakeSessionFactory) -> Arc<dyn PlatformDriver> {
    build_driver_with_config(
        factory,
        DriverConfig::from([("addr".into(), json!("192.168.8.43:35319"))]),
    )
}

fn build_driver_with_config(
    factory: FakeSessionFactory,
    driver_config: DriverConfig,
) -> Arc<dyn PlatformDriver> {
    HarmonyHdcDriverFactory::new_with_session_factory(Arc::new(factory))
        .build(&TargetDescriptor {
            id: TargetId("harmony-pc".into()),
            platform: "harmony".into(),
            driver: "harmony.hdc".into(),
            driver_config,
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

fn app_label(bundle_name: &str, label: &str) -> AppLabelInfo {
    AppLabelInfo {
        bundle_name: bundle_name.into(),
        label: label.into(),
    }
}

#[derive(Default)]
struct CallCounts {
    shell_connects: AtomicUsize,
    list_apps_calls: AtomicUsize,
    list_app_labels_calls: AtomicUsize,
    filter_desktop_bundles_calls: AtomicUsize,
    current_app_calls: AtomicUsize,
    list_windows_calls: AtomicUsize,
}

#[derive(Clone, Copy)]
enum ProbeOutcome {
    Ok,
    Err(&'static str),
    Sleep(Duration),
}

impl Default for ProbeOutcome {
    fn default() -> Self {
        Self::Ok
    }
}

impl ProbeOutcome {
    fn into_result(self) -> Result<(), operator_core::OperatorError> {
        match self {
            Self::Ok => Ok(()),
            Self::Err(message) => Err(operator_core::OperatorError::Platform(message.into())),
            Self::Sleep(duration) => {
                thread::sleep(duration);
                Ok(())
            }
        }
    }
}

#[derive(Clone)]
struct FakeSessionFactory {
    counts: Arc<CallCounts>,
    apps: Vec<String>,
    labels: Vec<AppLabelInfo>,
    desktop_visible_bundles: Vec<String>,
    current_app: Option<CurrentApp>,
    windows: CorrelatedWindowList,
    shell_connect: ProbeOutcome,
    shell_probe: ProbeOutcome,
    capture_probe: ProbeOutcome,
    ui_connect: ProbeOutcome,
    ui_probe: ProbeOutcome,
}

impl Default for FakeSessionFactory {
    fn default() -> Self {
        Self {
            counts: Arc::new(CallCounts::default()),
            apps: Vec::new(),
            labels: Vec::new(),
            desktop_visible_bundles: Vec::new(),
            current_app: None,
            windows: CorrelatedWindowList {
                windows: Vec::new(),
                focused_window_id: None,
                highlighted_window_ids: Vec::new(),
                total_window_count: Some(0),
            },
            shell_connect: ProbeOutcome::Ok,
            shell_probe: ProbeOutcome::Ok,
            capture_probe: ProbeOutcome::Ok,
            ui_connect: ProbeOutcome::Ok,
            ui_probe: ProbeOutcome::Ok,
        }
    }
}

impl HarmonyHdcSessionFactory for FakeSessionFactory {
    fn connect_shell(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcShellSession>, operator_core::OperatorError> {
        self.counts.shell_connects.fetch_add(1, Ordering::SeqCst);
        self.shell_connect.into_result()?;
        Ok(Box::new(FakeShellSession {
            counts: Arc::clone(&self.counts),
            apps: self.apps.clone(),
            labels: self.labels.clone(),
            desktop_visible_bundles: self.desktop_visible_bundles.clone(),
            current_app: self.current_app.clone(),
            windows: self.windows.clone(),
            shell_probe: self.shell_probe,
            capture_probe: self.capture_probe,
        }))
    }

    fn connect_ui(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, operator_core::OperatorError> {
        self.ui_connect.into_result()?;
        Ok(Box::new(FakeUiSession {
            probe: self.ui_probe,
        }))
    }
}

struct FakeShellSession {
    counts: Arc<CallCounts>,
    apps: Vec<String>,
    labels: Vec<AppLabelInfo>,
    desktop_visible_bundles: Vec<String>,
    current_app: Option<CurrentApp>,
    windows: CorrelatedWindowList,
    shell_probe: ProbeOutcome,
    capture_probe: ProbeOutcome,
}

impl HarmonyHdcShellSession for FakeShellSession {
    fn exec_checked(&mut self, _command: &str) -> Result<(), operator_core::OperatorError> {
        self.shell_probe.into_result()
    }

    fn screenshot_probe(&mut self) -> Result<(), operator_core::OperatorError> {
        self.capture_probe.into_result()
    }

    fn capture_screenshot(&mut self, _path: &Path) -> Result<(), operator_core::OperatorError> {
        self.capture_probe.into_result()
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
        self.counts.list_apps_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.apps.clone())
    }

    fn list_app_labels(&mut self) -> Result<Vec<AppLabelInfo>, operator_core::OperatorError> {
        self.counts
            .list_app_labels_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.labels.clone())
    }

    fn filter_desktop_bundles(
        &mut self,
        bundles: &[String],
    ) -> Result<Vec<String>, operator_core::OperatorError> {
        self.counts
            .filter_desktop_bundles_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(bundles
            .iter()
            .filter(|bundle| {
                self.desktop_visible_bundles
                    .iter()
                    .any(|item| item == *bundle)
            })
            .cloned()
            .collect())
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
        _point: operator_core::Point,
        _mode: operator_core::ClickMode,
    ) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn input_text(&mut self, _text: &str) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn press_keys(&mut self, _keys: &[u32]) -> Result<(), operator_core::OperatorError> {
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
        _from: operator_core::Point,
        _to: operator_core::Point,
        _speed: Option<u32>,
    ) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn swipe(
        &mut self,
        _from: operator_core::Point,
        _to: operator_core::Point,
        _speed: Option<u32>,
    ) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }
}

struct FakeUiSession {
    probe: ProbeOutcome,
}

impl HarmonyHdcUiSession for FakeUiSession {
    fn check_ready(&self) -> Result<(), operator_core::OperatorError> {
        self.probe.into_result()
    }

    fn resolve_locator(
        &mut self,
        _locator: &operator_core::Locator,
    ) -> Result<Option<operator_core::Point>, operator_core::OperatorError> {
        Ok(None)
    }
}
