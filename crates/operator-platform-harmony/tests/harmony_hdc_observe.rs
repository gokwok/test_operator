use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use hmdriver_rs::{CorrelatedWindowList, CurrentApp};
use image::{GenericImageView, ImageBuffer, Rgb};
use operator_core::{
    DriverConfig, ExecContext, ImageSizePx, ObserveRequest, PlatformDriver, Rect, Surface,
    SurfaceKind, TargetDescriptor, TargetId,
};
use operator_platform_harmony::{
    HarmonyHdcConfig, HarmonyHdcDriverFactory, HarmonyHdcSessionFactory, HarmonyHdcShellSession,
    HarmonyHdcUiSession,
};
use operator_runtime::PlatformDriverFactory;
use serde_json::{json, Value};
use tempfile::tempdir;

#[tokio::test]
async fn observe_frontmost_crops_to_focused_window_bounds() {
    let temp = tempdir().expect("tempdir");
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(
        FakeSessionFactory::new(
            Arc::clone(&counts),
            ImageSizePx {
                width: 120,
                height: 80,
            },
            Some(Rect {
                x: 10.0,
                y: 12.0,
                width: 40.0,
                height: 30.0,
            }),
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
            x: 10.0,
            y: 12.0,
            width: 40.0,
            height: 30.0,
        })
    );
    assert_eq!(
        snapshot.metadata.image_size_px,
        Some(ImageSizePx {
            width: 40,
            height: 30,
        })
    );
    assert!(snapshot.metadata.capture_duration_ms < 1_000);
    assert!(snapshot.elements.is_empty());
    assert!(snapshot.root_ids.is_empty());
    assert_eq!(
        image::open(temp.path().join(&artifact.0))
            .expect("cropped artifact")
            .dimensions(),
        (40, 30)
    );
    assert_eq!(counts.shell_connects.load(Ordering::SeqCst), 1);
    assert_eq!(counts.capture_calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.display_size_calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.focused_window_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn observe_frontmost_falls_back_to_fullscreen_when_focused_bounds_are_missing() {
    let temp = tempdir().expect("tempdir");
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(
        FakeSessionFactory::new(
            Arc::clone(&counts),
            ImageSizePx {
                width: 100,
                height: 60,
            },
            None,
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
        .expect("frontmost observe should still persist a screenshot");

    assert_eq!(
        snapshot.metadata.capture_bounds,
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 60.0,
        })
    );
    assert_eq!(
        snapshot.metadata.image_size_px,
        Some(ImageSizePx {
            width: 100,
            height: 60,
        })
    );
    assert_eq!(
        image::open(temp.path().join(&artifact.0))
            .expect("fullscreen fallback artifact")
            .dimensions(),
        (100, 60)
    );
    assert_eq!(counts.focused_window_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn observe_fullscreen_reuses_cached_shell_session() {
    let temp = tempdir().expect("tempdir");
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(
        FakeSessionFactory::new(
            Arc::clone(&counts),
            ImageSizePx {
                width: 256,
                height: 144,
            },
            Some(Rect {
                x: 12.0,
                y: 8.0,
                width: 80.0,
                height: 60.0,
            }),
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
            width: 256.0,
            height: 144.0,
        })
    );
    assert_eq!(
        first.snapshot.metadata.image_size_px,
        Some(ImageSizePx {
            width: 256,
            height: 144,
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
    assert_eq!(counts.focused_window_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn observe_frontmost_includes_filtered_compact_elements_when_requested() {
    let temp = tempdir().expect("tempdir");
    let counts = Arc::new(CallCounts::default());
    let driver = build_driver(
        FakeSessionFactory::new(
            Arc::clone(&counts),
            ImageSizePx {
                width: 120,
                height: 80,
            },
            Some(Rect {
                x: 10.0,
                y: 12.0,
                width: 40.0,
                height: 30.0,
            }),
        )
        .with_hierarchy(json!({
            "attributes": {},
            "children": [
                {
                    "attributes": {
                        "type": "Button",
                        "clickable": "true",
                        "bounds": "[12,15][38,35]"
                    },
                    "children": [{
                        "attributes": {
                            "type": "Text",
                            "text": "保留",
                            "bounds": "[14,18][28,28]"
                        }
                    }]
                },
                {
                    "attributes": {
                        "type": "Button",
                        "clickable": "true",
                        "bounds": "[80,10][110,40]"
                    },
                    "children": [{
                        "attributes": {
                            "type": "Text",
                            "text": "过滤",
                            "bounds": "[82,14][100,26]"
                        }
                    }]
                }
            ]
        })),
        temp.path(),
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
            &ExecContext {
                target: "harmony-pc".into(),
                session: None,
                timeout_ms: Some(500),
            },
        )
        .await
        .expect("observe should succeed");

    let snapshot = observed.snapshot;
    assert_eq!(snapshot.root_ids.len(), 1);
    let root = snapshot
        .elements
        .get(&snapshot.root_ids[0])
        .expect("filtered element should exist");
    assert_eq!(root.role, "window");
    assert_eq!(root.children.len(), 1);
    let kept = snapshot
        .elements
        .get(&root.children[0])
        .expect("filtered button should exist");
    assert_eq!(kept.role, "button");
    assert_eq!(kept.label.as_deref(), Some("保留"));
    assert_eq!(counts.dump_hierarchy_calls.load(Ordering::SeqCst), 1);
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
    focused_window_calls: AtomicUsize,
    dump_hierarchy_calls: AtomicUsize,
}

#[derive(Clone)]
struct FakeSessionFactory {
    counts: Arc<CallCounts>,
    image_size_px: ImageSizePx,
    focused_window_bounds: Option<Rect>,
    hierarchy: Value,
}

impl FakeSessionFactory {
    fn new(
        counts: Arc<CallCounts>,
        image_size_px: ImageSizePx,
        focused_window_bounds: Option<Rect>,
    ) -> Self {
        Self {
            counts,
            image_size_px,
            focused_window_bounds,
            hierarchy: json!({
                "attributes": {},
                "children": []
            }),
        }
    }

    fn with_hierarchy(mut self, hierarchy: Value) -> Self {
        self.hierarchy = hierarchy;
        self
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
            focused_window_bounds: self.focused_window_bounds,
            hierarchy: self.hierarchy.clone(),
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
    focused_window_bounds: Option<Rect>,
    hierarchy: Value,
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
        write_test_jpeg(path, self.image_size_px)?;
        Ok(())
    }

    fn display_size(&mut self) -> Result<ImageSizePx, operator_core::OperatorError> {
        self.counts
            .display_size_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.image_size_px)
    }

    fn focused_window_bounds(&mut self) -> Result<Option<Rect>, operator_core::OperatorError> {
        self.counts
            .focused_window_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.focused_window_bounds)
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

    fn dump_hierarchy(&mut self) -> Result<Value, operator_core::OperatorError> {
        self.counts
            .dump_hierarchy_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.hierarchy.clone())
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

struct FakeUiSession;

impl HarmonyHdcUiSession for FakeUiSession {
    fn check_ready(&self) -> Result<(), operator_core::OperatorError> {
        Ok(())
    }

    fn resolve_locator(
        &mut self,
        _locator: &operator_core::Locator,
    ) -> Result<Option<operator_core::Point>, operator_core::OperatorError> {
        Ok(None)
    }
}

fn write_test_jpeg(
    path: &Path,
    image_size_px: ImageSizePx,
) -> Result<(), operator_core::OperatorError> {
    let image = ImageBuffer::from_fn(image_size_px.width, image_size_px.height, |x, y| {
        Rgb([(x % 255) as u8, (y % 255) as u8, 127])
    });
    image
        .save_with_format(path, image::ImageFormat::Jpeg)
        .map_err(|error| {
            operator_core::OperatorError::Platform(format!(
                "failed to write test jpeg {}: {error}",
                path.display()
            ))
        })
}
