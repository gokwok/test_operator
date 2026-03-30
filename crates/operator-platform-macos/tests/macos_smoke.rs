use std::{process::Command, thread, time::Duration};

use operator_core::{
    Action, ActionFocusPolicy, ActionRequest, ActionTargetSelector, AppListMode, ClickMode,
    ExecContext, Locator, ObserveRequest, OperatorError, PermissionStatus, PermissionsReport,
    PlatformDriver, QueryRequest, QueryResult, Rect, Surface, SurfaceKind, WindowInfo,
};
use operator_platform_macos::MacosDriver;

fn default_action_request() -> ActionRequest {
    ActionRequest {
        action: Action::Move,
        locator: None,
        target_selector: None,
        focus_policy: ActionFocusPolicy::Auto,
        verifications: Vec::new(),
    }
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with screen recording and accessibility permission"]
async fn observe_frontmost_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "screen_recording") != Some(PermissionStatus::Granted)
        || permission_status(&health.permissions, "accessibility")
            != Some(PermissionStatus::Granted)
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

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility, screen recording, and Apple Events permissions"]
async fn click_and_type_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS input smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit smoke target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS input smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    for request in [
        ActionRequest {
            action: Action::Click {
                mode: ClickMode::Left,
            },
            locator: Some(Locator::Role {
                role: "AXTextArea".into(),
                index: 0,
            }),
            ..default_action_request()
        },
        ActionRequest {
            action: Action::Type {
                text: "operator smoke typing".into(),
                clear_before: false,
                delay_ms: None,
                trailing_keys: Vec::new(),
            },
            locator: None,
            ..default_action_request()
        },
    ] {
        match driver.act(request, &exec_context()).await {
            Ok(_) => {}
            Err(error) if is_sandboxed_macos_failure(&error) => {
                eprintln!("Skipping macOS input smoke test in sandboxed session: {error}");
                return;
            }
            Err(error) => panic!("input action failed: {error}"),
        }
    }

    thread::sleep(Duration::from_millis(500));

    let text = match read_textedit_document() {
        Ok(text) => text,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS input smoke verification in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("failed to read TextEdit document: {error}"),
    };

    assert!(
        text.contains("operator smoke typing"),
        "expected TextEdit front document to contain smoke text, got: {text:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn scroll_with_locator_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS scroll smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit scroll target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS scroll smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let outcome = match driver
        .act(
            ActionRequest {
                action: Action::Scroll {
                    delta_x: 0.0,
                    delta_y: -6.0,
                },
                locator: Some(Locator::Role {
                    role: "AXTextArea".into(),
                    index: 0,
                }),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS scroll smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("scroll action failed: {error}"),
    };

    assert!(outcome.success);

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn hotkey_with_system_driver_selects_all_and_replaces_text() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS hotkey smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit hotkey target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS hotkey smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let initial_text = "operator hotkey original";
    let replacement_text = "operator hotkey replaced";
    for request in [
        ActionRequest {
            action: Action::Click {
                mode: ClickMode::Left,
            },
            locator: Some(Locator::Role {
                role: "AXTextArea".into(),
                index: 0,
            }),
            ..default_action_request()
        },
        ActionRequest {
            action: Action::Type {
                text: initial_text.into(),
                clear_before: false,
                delay_ms: None,
                trailing_keys: Vec::new(),
            },
            locator: None,
            ..default_action_request()
        },
        ActionRequest {
            action: Action::Hotkey {
                keys: vec!["command".into(), "a".into()],
            },
            locator: None,
            ..default_action_request()
        },
        ActionRequest {
            action: Action::Type {
                text: replacement_text.into(),
                clear_before: false,
                delay_ms: None,
                trailing_keys: Vec::new(),
            },
            locator: None,
            ..default_action_request()
        },
    ] {
        match driver.act(request, &exec_context()).await {
            Ok(_) => {}
            Err(error) if is_sandboxed_macos_failure(&error) => {
                eprintln!("Skipping macOS hotkey smoke test in sandboxed session: {error}");
                return;
            }
            Err(error) => panic!("hotkey action failed: {error}"),
        }
    }

    thread::sleep(Duration::from_millis(500));

    let text = match read_textedit_document() {
        Ok(text) => text,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS hotkey smoke verification in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("failed to read TextEdit document: {error}"),
    };

    assert!(
        text.contains(replacement_text),
        "expected TextEdit front document to contain replacement text, got: {text:?}"
    );
    assert!(
        !text.contains(initial_text),
        "expected command-a hotkey to replace the original text, got: {text:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn list_windows_and_get_focus_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS focus smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit smoke target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS focus smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS focus smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    assert!(!windows.is_empty(), "expected at least one TextEdit window");
    assert!(
        windows.iter().all(|window| window.bounds.is_some()),
        "expected TextEdit windows to include bounds metadata: {windows:?}"
    );
    assert!(
        windows.iter().any(|window| window.is_focused),
        "expected one TextEdit window to be focused: {windows:?}"
    );

    let focus = match driver.query(QueryRequest::GetFocus, &exec_context()).await {
        Ok(focus) => focus,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS focus smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("focus query failed: {error}"),
    };

    let QueryResult::Focus(focus) = focus else {
        panic!("expected focus query result");
    };
    let focus = focus.expect("expected focused element info");
    assert_eq!(focus.app_name.as_deref(), Some("TextEdit"));
    assert!(!focus.role.is_empty(), "expected focused element role");

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn focus_window_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS focus-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare first TextEdit smoke target: {error}");
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS focus-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare second TextEdit smoke target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS focus-window smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS focus-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    assert!(
        windows.len() >= 2,
        "expected at least two TextEdit windows for focus-window smoke test: {windows:?}"
    );
    let target_window = windows
        .iter()
        .find(|window| !window.is_focused)
        .unwrap_or(&windows[0]);

    match driver
        .act(
            ActionRequest {
                action: Action::FocusWindow {
                    id: target_window.id,
                },
                locator: None,
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
    {
        Ok(_) => {}
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS focus-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("focus-window action failed: {error}"),
    }

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS focus-window smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after focus-window failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let focused = refreshed
        .iter()
        .find(|window| window.id == target_window.id && window.is_focused);
    assert!(
        focused.is_some(),
        "expected target window to become focused after focus-window: {refreshed:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn close_window_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS close-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare first TextEdit smoke target: {error}");
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS close-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare second TextEdit smoke target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS close-window smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS close-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    assert!(
        windows.len() >= 2,
        "expected at least two TextEdit windows for close-window smoke test: {windows:?}"
    );
    let initial_count = windows.len();
    let target_window = windows
        .iter()
        .find(|window| !window.is_focused)
        .unwrap_or(&windows[0]);

    match driver
        .act(
            ActionRequest {
                action: Action::CloseWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(_) => {}
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS close-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("close-window action failed: {error}"),
    }

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS close-window smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after close-window failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    assert_eq!(
        refreshed.len() + 1,
        initial_count,
        "expected one TextEdit window to close: {refreshed:?}"
    );
    assert!(
        refreshed.iter().all(|window| window.id != target_window.id),
        "expected target window to disappear after close-window: {refreshed:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn minimize_window_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS minimize-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit minimize target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS minimize-window smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS minimize-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    let target_window = windows
        .iter()
        .find(|window| !window.is_minimized)
        .unwrap_or_else(|| panic!("expected a non-minimized TextEdit window: {windows:?}"));

    match driver
        .act(
            ActionRequest {
                action: Action::MinimizeWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(_) => {}
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS minimize-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("minimize-window action failed: {error}"),
    }

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS minimize-window smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after minimize-window failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let minimized = refreshed
        .iter()
        .find(|window| window.id == target_window.id && window.is_minimized);
    assert!(
        minimized.is_some(),
        "expected target window to become minimized after minimize-window: {refreshed:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn maximize_window_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS maximize-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit maximize target: {error}");
    }

    if let Err(error) = set_textedit_front_window_bounds(80, 80, 420, 280) {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS maximize-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to resize TextEdit front window before maximize: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS maximize-window smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS maximize-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    let target_window = windows
        .iter()
        .find(|window| window.is_focused)
        .or_else(|| windows.first())
        .unwrap();
    let original_bounds = target_window
        .bounds
        .unwrap_or_else(|| panic!("expected TextEdit window bounds before maximize: {windows:?}"));

    match driver
        .act(
            ActionRequest {
                action: Action::MaximizeWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(_) => {}
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS maximize-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("maximize-window action failed: {error}"),
    }

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS maximize-window smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after maximize-window failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let target = refreshed
        .iter()
        .find(|window| window.id == target_window.id)
        .unwrap_or_else(|| panic!("expected target window after maximize-window: {refreshed:?}"));
    let refreshed_bounds = target.bounds.unwrap_or_else(|| {
        panic!("expected TextEdit window bounds after maximize-window: {refreshed:?}")
    });
    assert!(
        refreshed_bounds.width > original_bounds.width
            || refreshed_bounds.height > original_bounds.height,
        "expected maximize-window to expand the target bounds from {original_bounds:?} to {refreshed_bounds:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn move_window_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS move-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit move-window target: {error}");
    }

    if let Err(error) = set_textedit_front_window_bounds(80, 80, 420, 280) {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS move-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to resize TextEdit front window before move-window: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS move-window smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS move-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    let target_window = focused_or_first_window(&windows);

    let outcome = match driver
        .act(
            ActionRequest {
                action: Action::MoveWindow { x: 140.0, y: 160.0 },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS move-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("move-window action failed: {error}"),
    };
    let expected_detail = format!(
        "moved window {} to x=140 y=160 width=340 height=200",
        target_window.id
    );
    assert_eq!(outcome.detail.as_deref(), Some(expected_detail.as_str()));

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS move-window smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after move-window failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let target = refreshed
        .iter()
        .find(|window| window.id == target_window.id)
        .unwrap_or_else(|| panic!("expected target window after move-window: {refreshed:?}"));
    let bounds = target
        .bounds
        .unwrap_or_else(|| panic!("expected target bounds after move-window: {refreshed:?}"));
    assert!(
        rect_matches(
            bounds,
            Rect {
                x: 140.0,
                y: 160.0,
                width: 340.0,
                height: 200.0,
            },
            2.0
        ),
        "expected target bounds after move-window to be near x=140 y=160 width=340 height=200, got {bounds:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn resize_window_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS resize-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit resize-window target: {error}");
    }

    if let Err(error) = set_textedit_front_window_bounds(80, 80, 420, 280) {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS resize-window smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to resize TextEdit front window before resize-window: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS resize-window smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS resize-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    let target_window = focused_or_first_window(&windows);

    let outcome = match driver
        .act(
            ActionRequest {
                action: Action::ResizeWindow {
                    width: 520.0,
                    height: 360.0,
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS resize-window smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("resize-window action failed: {error}"),
    };
    let expected_detail = format!(
        "resized window {} to x=80 y=80 width=520 height=360",
        target_window.id
    );
    assert_eq!(outcome.detail.as_deref(), Some(expected_detail.as_str()));

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS resize-window smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after resize-window failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let target = refreshed
        .iter()
        .find(|window| window.id == target_window.id)
        .unwrap_or_else(|| panic!("expected target window after resize-window: {refreshed:?}"));
    let bounds = target
        .bounds
        .unwrap_or_else(|| panic!("expected target bounds after resize-window: {refreshed:?}"));
    assert!(
        rect_matches(
            bounds,
            Rect {
                x: 80.0,
                y: 80.0,
                width: 520.0,
                height: 360.0,
            },
            2.0
        ),
        "expected target bounds after resize-window to be near x=80 y=80 width=520 height=360, got {bounds:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn set_window_bounds_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS set-window-bounds smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit set-window-bounds target: {error}");
    }

    if let Err(error) = set_textedit_front_window_bounds(80, 80, 420, 280) {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS set-window-bounds smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to resize TextEdit front window before set-window-bounds: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS set-window-bounds smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS set-window-bounds smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    let target_window = focused_or_first_window(&windows);

    let outcome = match driver
        .act(
            ActionRequest {
                action: Action::SetWindowBounds {
                    bounds: Rect {
                        x: 120.0,
                        y: 140.0,
                        width: 460.0,
                        height: 320.0,
                    },
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS set-window-bounds smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("set-window-bounds action failed: {error}"),
    };
    let expected_detail = format!(
        "set window {} bounds to x=120 y=140 width=460 height=320",
        target_window.id
    );
    assert_eq!(outcome.detail.as_deref(), Some(expected_detail.as_str()));

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!(
                "Skipping macOS set-window-bounds smoke verification in sandboxed session: {error}"
            );
            return;
        }
        Err(error) => panic!("window query after set-window-bounds failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let target = refreshed
        .iter()
        .find(|window| window.id == target_window.id)
        .unwrap_or_else(|| panic!("expected target window after set-window-bounds: {refreshed:?}"));
    let bounds = target
        .bounds
        .unwrap_or_else(|| panic!("expected target bounds after set-window-bounds: {refreshed:?}"));
    assert!(
        rect_matches(
            bounds,
            Rect {
                x: 120.0,
                y: 140.0,
                width: 460.0,
                height: 320.0,
            },
            2.0
        ),
        "expected target bounds after set-window-bounds to be near x=120 y=140 width=460 height=320, got {bounds:?}"
    );

    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn switch_app_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS switch-app smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare TextEdit smoke target: {error}");
    }

    if let Err(error) = launch_calculator() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS switch-app smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare Calculator smoke target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let cleanup_calculator = CleanupCalculatorApp;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS switch-app smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let apps = match driver
        .query(
            QueryRequest::ListApps {
                mode: AppListMode::Running,
            },
            &exec_context(),
        )
        .await
    {
        Ok(apps) => apps,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS switch-app smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("app query failed: {error}"),
    };

    let QueryResult::Apps(apps) = apps else {
        panic!("expected apps query result");
    };
    let calculator_name = apps
        .iter()
        .find(|app| app.bundle_id.as_deref() == Some("com.apple.calculator"))
        .map(|app| app.name.clone())
        .unwrap_or_else(|| "Calculator".to_string());

    match driver
        .act(
            ActionRequest {
                action: Action::SwitchApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::App("com.apple.calculator".into())),
                ..default_action_request()
            },
            &exec_context(),
        )
        .await
    {
        Ok(_) => {}
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS switch-app smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("switch-app action failed: {error}"),
    }

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some(calculator_name.clone()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS switch-app smoke verification in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query after switch-app failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    assert!(
        refreshed.iter().any(|window| window.is_focused),
        "expected one {calculator_name} window to become focused after switch-app: {refreshed:?}"
    );

    drop(cleanup_calculator);
    drop(cleanup);
}

#[tokio::test]
#[ignore = "requires a macOS GUI session with accessibility and Apple Events permissions"]
async fn move_with_window_target_selector_with_system_driver() {
    if !cfg!(target_os = "macos") {
        eprintln!("Skipping macOS smoke test on non-macOS host.");
        return;
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS selector smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare first TextEdit selector target: {error}");
    }

    if let Err(error) = prepare_textedit_document() {
        if is_sandboxed_macos_failure(&error) {
            eprintln!("Skipping macOS selector smoke test in sandboxed session: {error}");
            return;
        }
        panic!("failed to prepare second TextEdit selector target: {error}");
    }

    let cleanup = CleanupTextEditDocument;
    let driver = MacosDriver::system();
    let health = driver.health_check().await.unwrap();
    if permission_status(&health.permissions, "accessibility") != Some(PermissionStatus::Granted) {
        eprintln!(
            "Skipping macOS selector smoke test without accessibility permission: {:?}",
            health.permissions
        );
        return;
    }

    thread::sleep(Duration::from_millis(500));

    let windows = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS selector smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query failed: {error}"),
    };

    let QueryResult::Windows(windows) = windows else {
        panic!("expected windows query result");
    };
    assert!(
        windows.len() >= 2,
        "expected at least two TextEdit windows for selector smoke test: {windows:?}"
    );
    let target_window = windows
        .iter()
        .find(|window| !window.is_focused)
        .unwrap_or(&windows[0]);

    match driver
        .act(
            ActionRequest {
                action: Action::Move,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowId(target_window.id)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            &exec_context(),
        )
        .await
    {
        Ok(_) => {}
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS selector smoke test in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("move action with selector failed: {error}"),
    }

    thread::sleep(Duration::from_millis(500));

    let refreshed = match driver
        .query(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            &exec_context(),
        )
        .await
    {
        Ok(windows) => windows,
        Err(error) if is_sandboxed_macos_failure(&error) => {
            eprintln!("Skipping macOS selector smoke verification in sandboxed session: {error}");
            return;
        }
        Err(error) => panic!("window query after selector move failed: {error}"),
    };

    let QueryResult::Windows(refreshed) = refreshed else {
        panic!("expected windows query result");
    };
    let focused = refreshed
        .iter()
        .find(|window| window.id == target_window.id && window.is_focused);
    assert!(
        focused.is_some(),
        "expected target window to become focused after selector move: {refreshed:?}"
    );

    drop(cleanup);
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
                || message.contains("Not authorized to send Apple events")
                || message.contains("not allowed to send keystrokes")
        }
        _ => false,
    }
}

struct CleanupTextEditDocument;

impl Drop for CleanupTextEditDocument {
    fn drop(&mut self) {
        let _ = run_osascript(
            r#"
tell application "TextEdit"
  repeat while (count of documents) > 0
    close front document saving no
  end repeat
end tell
"#,
        );
    }
}

struct CleanupCalculatorApp;

impl Drop for CleanupCalculatorApp {
    fn drop(&mut self) {
        let _ = run_osascript(
            r#"
tell application id "com.apple.calculator"
  quit
end tell
"#,
        );
    }
}

fn prepare_textedit_document() -> Result<(), OperatorError> {
    run_osascript(
        r#"
tell application "TextEdit"
  activate
end tell

delay 0.5

tell application "TextEdit"
  make new document
end tell
"#,
    )?;
    Ok(())
}

fn launch_calculator() -> Result<(), OperatorError> {
    run_osascript(
        r#"
tell application id "com.apple.calculator"
  activate
end tell
"#,
    )?;
    Ok(())
}

fn set_textedit_front_window_bounds(
    x: i32,
    y: i32,
    right: i32,
    bottom: i32,
) -> Result<(), OperatorError> {
    run_osascript(&format!(
        r#"
tell application "TextEdit"
  activate
  set bounds of front window to {{{x}, {y}, {right}, {bottom}}}
end tell
"#,
    ))?;
    Ok(())
}

fn focused_or_first_window(windows: &[WindowInfo]) -> &WindowInfo {
    windows
        .iter()
        .find(|window| window.is_focused)
        .or_else(|| windows.first())
        .unwrap_or_else(|| panic!("expected at least one TextEdit window: {windows:?}"))
}

fn rect_matches(actual: Rect, expected: Rect, tolerance: f64) -> bool {
    (actual.x - expected.x).abs() <= tolerance
        && (actual.y - expected.y).abs() <= tolerance
        && (actual.width - expected.width).abs() <= tolerance
        && (actual.height - expected.height).abs() <= tolerance
}

fn read_textedit_document() -> Result<String, OperatorError> {
    run_osascript(
        r#"
tell application "TextEdit"
  activate
  text of front document
end tell
"#,
    )
}

fn run_osascript(script: &str) -> Result<String, OperatorError> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|error| OperatorError::Platform(format!("failed to invoke osascript: {error}")))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("Not authorized") || stderr.contains("not allowed") {
        return Err(OperatorError::PermissionDenied(stderr));
    }

    Err(OperatorError::Platform(format!(
        "osascript failed: {stderr}"
    )))
}

fn permission_status(report: &PermissionsReport, id: &str) -> Option<PermissionStatus> {
    report.status(id)
}
