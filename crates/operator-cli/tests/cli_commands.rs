#[path = "../src/main.rs"]
mod cli_main;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use clap::Parser;
use serde_json::{json, Value};

#[test]
fn observe_command_supports_json_flag() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "observe",
        "--surface",
        "frontmost",
        "--json",
    ])
    .unwrap();

    assert!(cli.prefers_json());

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "observe");
    assert_eq!(
        invocation.input,
        json!({
            "surface": {
                "kind": "Frontmost"
            },
            "include_screenshot": false,
            "include_elements": false
        })
    );
}

#[test]
fn artifact_get_command_maps_artifact_id_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "artifact-get",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--artifact-id",
        "capture-1.png",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "artifact-get");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "artifact_id": "capture-1.png"
        })
    );
}

#[test]
fn click_command_maps_snapshot_flags_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "click",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--snapshot",
        "s_123",
        "--element",
        "e_45",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "click");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "mode": "Left",
            "locator": {
                "SnapshotElement": {
                    "snapshot": "s_123",
                    "element": "e_45"
                }
            }
        })
    );
}

#[test]
fn click_command_accepts_explicit_mode_without_locator() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "click",
        "--target",
        "local:macos",
        "--mode",
        "double",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "click");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "mode": "Double"
        })
    );
}

#[test]
fn click_command_maps_window_target_selector_and_focus_policy() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "click",
        "--target",
        "local:macos",
        "--mode",
        "double",
        "--window-title",
        "Project Notes",
        "--focus-policy",
        "never",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "click");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "mode": "Double",
            "target_selector": {
                "WindowTitle": "Project Notes"
            },
            "focus_policy": "Never"
        })
    );
}

#[test]
fn get_focus_command_maps_common_flags_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "get-focus",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "get-focus");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250
        })
    );
}

#[tokio::test]
async fn focus_window_command_maps_window_id_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "focus-window",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--window-id",
        "42",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "focus-window");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "window_id": 42
        })
    );
}

#[tokio::test]
async fn type_command_maps_app_target_selector_with_default_focus_policy() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "type",
        "--target",
        "local:macos",
        "--text",
        "hello operator",
        "--app",
        "TextEdit",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "type");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "text": "hello operator",
            "clear_before": false,
            "target_selector": {
                "App": "TextEdit"
            },
            "focus_policy": "Auto"
        })
    );
}

#[tokio::test]
async fn switch_app_command_maps_app_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "switch-app",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--app",
        "TextEdit",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "switch-app");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "target_selector": {
                "App": "TextEdit"
            }
        })
    );
}

#[tokio::test]
async fn quit_app_command_maps_pid_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "quit-app",
        "--target",
        "local:macos",
        "--pid",
        "101",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "quit-app");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "target_selector": {
                "Pid": 101
            }
        })
    );
}

#[tokio::test]
async fn relaunch_app_command_maps_window_title_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "relaunch-app",
        "--window-title",
        "Draft",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "relaunch-app");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "WindowTitle": "Draft"
            }
        })
    );
}

#[tokio::test]
async fn hide_app_command_maps_window_index_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "hide-app", "--window-index", "1"])
        .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "hide-app");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "WindowIndex": 1
            }
        })
    );
}

#[tokio::test]
async fn unhide_app_command_maps_window_id_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "unhide-app", "--window-id", "42"])
        .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "unhide-app");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "WindowId": 42
            }
        })
    );
}

#[tokio::test]
async fn scroll_command_maps_deltas_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "scroll",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--delta-x",
        "0",
        "--delta-y",
        "-120",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "scroll");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "delta_x": 0.0,
            "delta_y": -120.0
        })
    );
}

#[tokio::test]
async fn scroll_command_accepts_snapshot_locator() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "scroll",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--delta-x",
        "0",
        "--delta-y",
        "-120",
        "--snapshot",
        "s_123",
        "--element",
        "e_45",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "scroll");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "delta_x": 0.0,
            "delta_y": -120.0,
            "locator": {
                "SnapshotElement": {
                    "snapshot": "s_123",
                    "element": "e_45"
                }
            }
        })
    );
}

#[tokio::test]
async fn move_command_maps_coordinate_target_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "move",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--x",
        "640",
        "--y",
        "480",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "move");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "locator": {
                "Coords": {
                    "x": 640.0,
                    "y": 480.0
                }
            }
        })
    );
}

#[tokio::test]
async fn drag_command_maps_from_and_to_locators_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "drag",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--from-snapshot",
        "s_123",
        "--from-element",
        "e_45",
        "--to-x",
        "640",
        "--to-y",
        "480",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "drag");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "from": {
                "SnapshotElement": {
                    "snapshot": "s_123",
                    "element": "e_45"
                }
            },
            "to": {
                "Coords": {
                    "x": 640.0,
                    "y": 480.0
                }
            }
        })
    );
}

#[tokio::test]
async fn drag_command_maps_motion_options_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "drag",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--from-x",
        "12",
        "--from-y",
        "24",
        "--to-x",
        "640",
        "--to-y",
        "480",
        "--duration-ms",
        "300",
        "--steps",
        "6",
        "--modifier",
        "command",
        "--modifier",
        "shift",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "drag");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "from": {
                "Coords": {
                    "x": 12.0,
                    "y": 24.0
                }
            },
            "to": {
                "Coords": {
                    "x": 640.0,
                    "y": 480.0
                }
            },
            "duration_ms": 300,
            "steps": 6,
            "modifiers": ["Command", "Shift"]
        })
    );
}

#[tokio::test]
async fn swipe_command_maps_motion_options_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "swipe",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--from-x",
        "12",
        "--from-y",
        "24",
        "--to-x",
        "640",
        "--to-y",
        "480",
        "--duration-ms",
        "300",
        "--steps",
        "6",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "swipe");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "from": {
                "Coords": {
                    "x": 12.0,
                    "y": 24.0
                }
            },
            "to": {
                "Coords": {
                    "x": 640.0,
                    "y": 480.0
                }
            },
            "duration_ms": 300,
            "steps": 6
        })
    );
}

#[tokio::test]
async fn hotkey_command_maps_repeated_key_flags_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "hotkey",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--key",
        "command",
        "--key",
        "shift",
        "--key",
        "p",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "hotkey");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "keys": ["command", "shift", "p"]
        })
    );
}

#[tokio::test]
async fn press_command_maps_key_and_count_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "press",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--key",
        "down",
        "--count",
        "3",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "press");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "key": "down",
            "count": 3
        })
    );
}

#[tokio::test]
async fn type_command_maps_clear_delay_and_trailing_keys_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "type",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--text",
        "hello world",
        "--clear-before",
        "--delay-ms",
        "25",
        "--trailing-key",
        "return",
        "--trailing-key",
        "tab",
        "--locator-text",
        "Search",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "type");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "text": "hello world",
            "clear_before": true,
            "delay_ms": 25,
            "trailing_keys": ["Return", "Tab"],
            "locator": {
                "Text": "Search"
            }
        })
    );
}

#[tokio::test]
async fn cli_run_renders_move_action_for_non_json_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "move",
        "--target",
        "local:macos",
        "--x",
        "24",
        "--y",
        "48",
    ])
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 7,
                "detail": "moved"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "moved");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "move");
    assert_eq!(
        recorded[0].1,
        json!({
            "target": "local:macos",
            "locator": {
                "Coords": {
                    "x": 24.0,
                    "y": 48.0
                }
            }
        })
    );
}

#[tokio::test]
async fn cli_run_forwards_tool_calls_and_renders_json_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "launch-app",
        "--json",
        "--target",
        "local:macos",
        "--bundle-id-or-name",
        "Calculator",
    ])
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 9,
                "detail": "launched"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    let output = serde_json::from_str::<Value>(&rendered).unwrap();
    assert_eq!(output["outcome"]["detail"], json!("launched"));

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, "launch-app");
    assert_eq!(
        recorded[0].1,
        json!({
            "target": "local:macos",
            "bundle_id_or_name": "Calculator"
        })
    );
}

#[tokio::test]
async fn cli_run_renders_switch_app_detail_for_non_json_output() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "switch-app", "--app", "TextEdit"])
        .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 10,
                "detail": "switched app"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "switched app");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "switch-app");
    assert_eq!(
        recorded[0].1,
        json!({
            "target_selector": {
                "App": "TextEdit"
            }
        })
    );
}

#[tokio::test]
async fn cli_run_renders_press_detail_for_non_json_output() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "press", "--key", "down", "--count", "3"])
            .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 7,
                "detail": "pressed down 3 times"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "pressed down 3 times");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "press");
    assert_eq!(recorded[0].1, json!({ "key": "down", "count": 3 }));
}

#[tokio::test]
async fn cli_run_renders_artifact_path_for_non_json_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "artifact-get",
        "--artifact-id",
        "capture-1.png",
    ])
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "artifact": {
                "id": "capture-1.png",
                "path": "/tmp/operator/artifacts/capture-1.png"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(
        rendered,
        "artifact capture-1.png (/tmp/operator/artifacts/capture-1.png)"
    );

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "artifact-get");
    assert_eq!(recorded[0].1, json!({ "artifact_id": "capture-1.png" }));
}

#[tokio::test]
async fn cli_run_renders_swipe_detail_for_non_json_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "swipe", "--from-x", "10", "--from-y", "20", "--to-x", "100", "--to-y", "20",
    ])
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 15,
                "detail": "swiped"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "swiped");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "swipe");
}

struct RecordingInvoker {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    response: Value,
}

impl cli_main::ToolInvoker for RecordingInvoker {
    fn invoke<'a>(
        &'a self,
        tool: &'a str,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, operator_core::OperatorError>> + Send + 'a>>
    {
        self.calls
            .lock()
            .unwrap()
            .push((tool.to_string(), input.clone()));
        let response = self.response.clone();
        Box::pin(async move { Ok(response) })
    }
}
