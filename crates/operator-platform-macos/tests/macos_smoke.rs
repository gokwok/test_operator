use operator_core::{
    ExecContext, ObserveRequest, OperatorError, PermissionStatus, PlatformDriver, Surface,
    SurfaceKind,
};
use operator_platform_macos::MacosDriver;

#[tokio::test]
#[ignore = "requires a macOS GUI session with screen recording and accessibility permission"]
async fn observe_frontmost_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if health.permissions.screen_recording != PermissionStatus::Granted
        || health.permissions.accessibility != PermissionStatus::Granted
    {
        eprintln!(
            "Skipping macOS smoke test without required permissions: {:?}",
            health.permissions
        );
        return;
    }

    let observed = match driver
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
    {
        Ok(observed) => observed,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("observe failed: {error}"),
    };

    assert_eq!(observed.snapshot.target, "local:macos".into());
    assert_eq!(
        observed.snapshot.surface,
        Surface {
            kind: SurfaceKind::Frontmost,
        }
    );
    assert_eq!(observed.snapshot.metadata.platform, "macos");
    assert!(observed.snapshot.image_artifact.is_some());
    assert!(!observed.snapshot.root_ids.is_empty());
}

fn exec_context() -> ExecContext {
    ExecContext {
        target: "local:macos".into(),
        session: None,
        timeout_ms: Some(5_000),
    }
}

fn is_sandboxed_macos_failure(error: &OperatorError) -> bool {
    match error {
        OperatorError::Platform(message) | OperatorError::PermissionDenied(message) => {
            message.contains("could not create image from display")
                || message.contains("Connection invalid")
                || message.contains("Application can't be found")
                || message.contains("-10827")
        }
        _ => false,
    }
}
