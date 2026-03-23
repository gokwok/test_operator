use std::{process::Command, thread, time::Duration};

use operator_core::{
    Action, ActionFocusPolicy, ActionRequest, ActionTargetSelector, ClickMode, ExecContext,
    Locator, ObserveRequest, OperatorError, PermissionStatus, PlatformDriver, QueryRequest,
    QueryResult, Surface, SurfaceKind,
};
use operator_platform_macos::MacosDriver;

fn default_action_request() -> ActionRequest {
    ActionRequest {
        action: Action::Move,
        locator: None,
        target_selector: None,
        focus_policy: ActionFocusPolicy::Auto,
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
    if health.permissions.accessibility != PermissionStatus::Granted {
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
    if health.permissions.accessibility != PermissionStatus::Granted {
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
    if health.permissions.accessibility != PermissionStatus::Granted {
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
    if health.permissions.accessibility != PermissionStatus::Granted {
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
    if health.permissions.accessibility != PermissionStatus::Granted {
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
    if health.permissions.accessibility != PermissionStatus::Granted {
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
