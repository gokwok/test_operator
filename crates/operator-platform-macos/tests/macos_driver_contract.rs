use std::sync::Mutex;

use operator_core::{
    Action, ActionRequest, AppInfo, Capability, ExecContext, OperatorError, PermissionStatus,
    PermissionsReport, PlatformDriver, QueryRequest, QueryResult, WindowInfo,
};
use operator_platform_macos::{AppService, MacosDriver, PermissionReader};

#[test]
fn macos_driver_declares_expected_capabilities() {
    let driver = MacosDriver::new(StubAppService::default(), StubPermissionReader::granted());
    let capabilities = driver.capabilities();

    assert!(capabilities.supports(&Capability::AppLifecycle));
    assert!(capabilities.supports(&Capability::WindowManagement));
    assert!(capabilities.supports(&Capability::Permissions));
    assert!(!capabilities.supports(&Capability::Capture));
    assert!(!capabilities.supports(&Capability::InspectTree));
    assert!(!capabilities.supports(&Capability::PointerInput));
    assert!(!capabilities.supports(&Capability::KeyboardInput));
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
            launched: Mutex::new(Vec::new()),
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
    launched: Mutex<Vec<String>>,
    last_window_filter: Mutex<Option<String>>,
}

impl StubAppService {
    fn launched_apps(&self) -> Vec<String> {
        self.launched.lock().unwrap().clone()
    }

    fn last_window_filter(&self) -> Option<String> {
        self.last_window_filter.lock().unwrap().clone()
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

    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.launched
            .lock()
            .unwrap()
            .push(bundle_id_or_name.to_string());
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
