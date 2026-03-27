use std::path::Path;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use hmdriver_rs::{CorrelatedWindowList, CurrentApp};
use operator_core::{
    DriverConfig, ImageSizePx, PermissionStatus, PlatformDriver, Rect, TargetDescriptor, TargetId,
};
use operator_platform_harmony::{
    HarmonyHdcConfig, HarmonyHdcDriverFactory, HarmonyHdcSessionFactory, HarmonyHdcShellSession,
    HarmonyHdcUiSession, HDC_CAPTURE_CHECK_ID, HDC_CONNECT_CHECK_ID, HDC_SHELL_CHECK_ID,
    HDC_UI_BRIDGE_CHECK_ID,
};
use operator_runtime::PlatformDriverFactory;
use serde_json::json;

#[test]
fn config_normalizes_defaulted_fields() {
    let config = HarmonyHdcConfig::try_from(&DriverConfig::from([(
        "addr".into(),
        json!("192.168.8.43:35319"),
    )]))
    .expect("config should parse");

    assert_eq!(config.connect_key(), "192.168.8.43:35319");
    assert_eq!(config.remote_agent_path(), "/data/local/tmp/agent.so");
    assert_eq!(config.timeout().as_millis(), 60_000);
    assert_eq!(config.startup_delay().as_millis(), 500);
}

#[tokio::test]
async fn health_check_reuses_cached_shell_and_ui_sessions() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory::success(Arc::clone(&counts)));

    let first = driver.health_check().await.expect("first health check");
    let second = driver.health_check().await.expect("second health check");

    assert!(first.healthy);
    assert!(second.healthy);
    assert_eq!(
        first.permissions.status(HDC_CONNECT_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(
        first.permissions.status(HDC_SHELL_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(
        first.permissions.status(HDC_CAPTURE_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(
        first.permissions.status(HDC_UI_BRIDGE_CHECK_ID),
        Some(PermissionStatus::Granted)
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.shell_probes.load(Ordering::SeqCst), 2);
    assert_eq!(counts.capture_probes.load(Ordering::SeqCst), 2);
    assert_eq!(counts.ui_probes.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn health_check_reports_ui_bridge_limit_without_marking_driver_unhealthy() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        shell_connect: ProbeOutcome::Ok,
        shell_probe: ProbeOutcome::Ok,
        capture_probe: ProbeOutcome::Ok,
        ui_connect: ProbeOutcome::Err("ui bridge unavailable"),
        ui_probe: ProbeOutcome::Ok,
    });

    let health = driver.health_check().await.expect("health check");

    assert!(health.healthy);
    assert!(health
        .message
        .as_deref()
        .is_some_and(|message| message.contains("ui bridge unavailable")));
    assert_eq!(
        health.permissions.status(HDC_UI_BRIDGE_CHECK_ID),
        Some(PermissionStatus::Denied)
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn health_check_marks_driver_unhealthy_when_connect_fails() {
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(FakeSessionFactory {
        counts: Arc::clone(&counts),
        shell_connect: ProbeOutcome::Err("tcp connect failed"),
        shell_probe: ProbeOutcome::Ok,
        capture_probe: ProbeOutcome::Ok,
        ui_connect: ProbeOutcome::Ok,
        ui_probe: ProbeOutcome::Ok,
    });

    let health = driver.health_check().await.expect("health check");

    assert!(!health.healthy);
    assert!(health
        .message
        .as_deref()
        .is_some_and(|message| message.contains("tcp connect failed")));
    assert_eq!(
        health.permissions.status(HDC_CONNECT_CHECK_ID),
        Some(PermissionStatus::Denied)
    );
    assert_eq!(
        health.permissions.status(HDC_SHELL_CHECK_ID),
        Some(PermissionStatus::NotDetermined)
    );
    assert_eq!(
        health.permissions.status(HDC_UI_BRIDGE_CHECK_ID),
        Some(PermissionStatus::NotDetermined)
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ui_connects.load(Ordering::SeqCst), 0);
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

#[derive(Default)]
struct CallCounts {
    shell_connects: AtomicUsize,
    ui_connects: AtomicUsize,
    shell_probes: AtomicUsize,
    capture_probes: AtomicUsize,
    ui_probes: AtomicUsize,
}

#[derive(Clone, Copy)]
enum ProbeOutcome {
    Ok,
    Err(&'static str),
}

impl ProbeOutcome {
    fn into_result(self) -> Result<(), operator_core::OperatorError> {
        match self {
            Self::Ok => Ok(()),
            Self::Err(message) => Err(operator_core::OperatorError::Platform(message.into())),
        }
    }
}

#[derive(Clone)]
struct FakeSessionFactory {
    counts: Arc<CallCounts>,
    shell_connect: ProbeOutcome,
    shell_probe: ProbeOutcome,
    capture_probe: ProbeOutcome,
    ui_connect: ProbeOutcome,
    ui_probe: ProbeOutcome,
}

impl FakeSessionFactory {
    fn success(counts: Arc<CallCounts>) -> Self {
        Self {
            counts,
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
            shell_probe: self.shell_probe,
            capture_probe: self.capture_probe,
        }))
    }

    fn connect_ui(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, operator_core::OperatorError> {
        self.counts.ui_connects.fetch_add(1, Ordering::SeqCst);
        self.ui_connect.into_result()?;
        Ok(Box::new(FakeUiSession {
            counts: Arc::clone(&self.counts),
            probe: self.ui_probe,
        }))
    }
}

struct FakeShellSession {
    counts: Arc<CallCounts>,
    shell_probe: ProbeOutcome,
    capture_probe: ProbeOutcome,
}

impl HarmonyHdcShellSession for FakeShellSession {
    fn exec_checked(&mut self, _command: &str) -> Result<(), operator_core::OperatorError> {
        self.counts.shell_probes.fetch_add(1, Ordering::SeqCst);
        self.shell_probe.into_result()
    }

    fn screenshot_probe(&mut self) -> Result<(), operator_core::OperatorError> {
        self.counts.capture_probes.fetch_add(1, Ordering::SeqCst);
        self.capture_probe.into_result()
    }

    fn capture_screenshot(&mut self, _path: &Path) -> Result<(), operator_core::OperatorError> {
        self.counts.capture_probes.fetch_add(1, Ordering::SeqCst);
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
        Ok(Vec::new())
    }

    fn current_app(&mut self) -> Result<Option<CurrentApp>, operator_core::OperatorError> {
        Ok(None)
    }

    fn list_windows_with_missions(
        &mut self,
    ) -> Result<CorrelatedWindowList, operator_core::OperatorError> {
        Ok(CorrelatedWindowList {
            windows: Vec::new(),
            focused_window_id: None,
            highlighted_window_ids: Vec::new(),
            total_window_count: Some(0),
        })
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
    counts: Arc<CallCounts>,
    probe: ProbeOutcome,
}

impl HarmonyHdcUiSession for FakeUiSession {
    fn check_ready(&self) -> Result<(), operator_core::OperatorError> {
        self.counts.ui_probes.fetch_add(1, Ordering::SeqCst);
        self.probe.into_result()
    }

    fn resolve_locator(
        &mut self,
        _locator: &operator_core::Locator,
    ) -> Result<Option<operator_core::Point>, operator_core::OperatorError> {
        Ok(None)
    }
}
