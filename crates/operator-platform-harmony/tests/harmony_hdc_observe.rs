use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use operator_core::{
    DriverConfig, ExecContext, ImageSizePx, ObserveRequest, PlatformDriver, Rect, Surface,
    SurfaceKind, TargetDescriptor, TargetId,
};
use operator_platform_harmony::{
    HarmonyHdcConfig, HarmonyHdcDriverFactory, HarmonyHdcSessionFactory, HarmonyHdcShellSession,
    HarmonyHdcUiSession,
};
use operator_runtime::PlatformDriverFactory;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn observe_frontmost_persists_screenshot_and_normalized_metadata() {
    let temp = tempdir().expect("tempdir");
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(
        FakeSessionFactory::new(
            Arc::clone(&counts),
            ImageSizePx {
                width: 1920,
                height: 1080,
            },
        ),
        temp.path(),
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
            &ExecContext {
                target: "harmony-pc".into(),
                session: None,
                timeout_ms: Some(500),
            },
        )
        .await
        .expect("observe should succeed");

    let snapshot = observed.snapshot;
    let artifact = snapshot
        .image_artifact
        .clone()
        .expect("frontmost observe should persist a screenshot");

    assert_eq!(snapshot.target, TargetId("harmony-pc".into()));
    assert_eq!(snapshot.metadata.platform, "harmony");
    assert_eq!(
        snapshot.metadata.capture_bounds,
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        })
    );
    assert_eq!(
        snapshot.metadata.image_size_px,
        Some(ImageSizePx {
            width: 1920,
            height: 1080,
        })
    );
    assert!(snapshot.metadata.capture_duration_ms < 1_000);
    assert!(snapshot.elements.is_empty());
    assert!(snapshot.root_ids.is_empty());
    assert_eq!(
        std::fs::read(temp.path().join(&artifact.0)).expect("artifact bytes"),
        b"fake-jpeg"
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.capture_calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.display_size_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn observe_fullscreen_reuses_cached_shell_session() {
    let temp = tempdir().expect("tempdir");
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(
        FakeSessionFactory::new(
            Arc::clone(&counts),
            ImageSizePx {
                width: 2560,
                height: 1440,
            },
        ),
        temp.path(),
    );
    let request = ObserveRequest {
        surface: Surface {
            kind: SurfaceKind::Fullscreen {
                display_id: Some(7),
            },
        },
        include_screenshot: true,
        include_elements: false,
    };
    let ctx = ExecContext {
        target: "harmony-pc".into(),
        session: None,
        timeout_ms: Some(500),
    };

    let first = driver
        .observe(request.clone(), &ctx)
        .await
        .expect("first observe should succeed");
    let second = driver
        .observe(request, &ctx)
        .await
        .expect("second observe should succeed");

    assert_eq!(
        first.snapshot.metadata.capture_bounds,
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        })
    );
    assert_eq!(
        second.snapshot.surface.kind,
        SurfaceKind::Fullscreen {
            display_id: Some(7)
        }
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.capture_calls.load(Ordering::SeqCst), 2);
    assert_eq!(counts.display_size_calls.load(Ordering::SeqCst), 2);
}

fn build_driver(factory: FakeSessionFactory, artifacts_dir: &Path) -> Arc<dyn PlatformDriver> {
    HarmonyHdcDriverFactory::new_with_session_factory_and_artifacts_dir(
        Arc::new(factory),
        artifacts_dir,
    )
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
    capture_calls: AtomicUsize,
    display_size_calls: AtomicUsize,
}

#[derive(Clone)]
struct FakeSessionFactory {
    counts: Arc<CallCounts>,
    image_size_px: ImageSizePx,
}

impl FakeSessionFactory {
    fn new(counts: Arc<CallCounts>, image_size_px: ImageSizePx) -> Self {
        Self {
            counts,
            image_size_px,
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
            image_size_px: self.image_size_px,
        }))
    }

    fn connect_ui(
        &self,
        _config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, operator_core::OperatorError> {
        Ok(Box::new(FakeUiSession))
    }
}

struct FakeShellSession {
    counts: Arc<CallCounts>,
    image_size_px: ImageSizePx,
}

impl HarmonyHdcShellSession for FakeShellSession {
    fn exec_checked(&mut self, _command: &str) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn screenshot_probe(&mut self) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn capture_screenshot(&mut self, path: &Path) -> Result<(), operator_core::OperatorError> {
        self.counts.capture_calls.fetch_add(1, Ordering::SeqCst);
        std::fs::write(path, b"fake-jpeg")?;
        Ok(())
    }

    fn display_size(&mut self) -> Result<ImageSizePx, operator_core::OperatorError> {
        self.counts
            .display_size_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.image_size_px)
    }
}

struct FakeUiSession;

impl HarmonyHdcUiSession for FakeUiSession {
    fn check_ready(&self) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }
}
