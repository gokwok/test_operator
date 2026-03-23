#[path = "../src/main.rs"]
mod cli_main;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

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
fn permissions_command_uses_root_global_runtime_flags() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "--json",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "permissions",
    ])
    .unwrap();

    assert!(cli.prefers_json());

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "permissions-status");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250
        })
    );
}

#[test]
fn focus_command_maps_common_flags_to_internal_tool() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "focus",
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

#[test]
fn list_apps_command_moves_under_list_group() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "list", "apps"]).unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-apps");
    assert_eq!(invocation.input, json!({}));
}

#[test]
fn list_windows_command_moves_under_list_group() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "list", "windows", "--app", "TextEdit"])
            .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-windows");
    assert_eq!(
        invocation.input,
        json!({
            "app": "TextEdit"
        })
    );
}

#[test]
fn grouped_top_level_command_placeholders_exist() {
    for command in ["snapshot", "artifact", "input", "app", "window", "mcp"] {
        let cli = cli_main::args::Cli::try_parse_from(["operator", command]).unwrap();
        let error = cli.into_invocation().unwrap_err();
        assert!(
            error.contains("not implemented"),
            "unexpected error for {command}: {error}"
        );
    }
}

#[test]
fn root_help_groups_commands_by_domain() {
    let help = cli_main::args::Cli::command().render_help().to_string();
    assert_eq!(
        help,
        "Operator automation CLI\n\n\
Usage: operator [OPTIONS] [COMMAND]\n\n\
Core:\n  permissions   Inspect platform permission state\n  capabilities  List runtime capabilities\n\n\
Observe:\n  observe       Capture UI state\n  snapshot      Work with persisted snapshots\n  artifact      Work with persisted artifacts\n\n\
Query:\n  list          Enumerate apps and windows\n  focus         Inspect current focus\n\n\
Action:\n  input         Pointer and keyboard actions\n  app           Application lifecycle actions\n  window        Window management actions\n\n\
MCP:\n  mcp           MCP server commands\n\n\
A2A:\n  reserved      Reserved for future A2A commands\n\n\
Options:\n      --json                   Render structured JSON output\n      --target <TARGET>        Select a runtime target\n      --timeout-ms <TIMEOUT_MS>\n                               Override runtime timeout in milliseconds\n  -h, --help                   Print help\n"
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
        "--verify",
        "focus",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "focus-window");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "window_id": 42,
            "verifications": ["Focus"]
        })
    );
}

#[tokio::test]
async fn close_window_command_maps_window_target_selector_and_focus_policy() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "close-window",
        "--target",
        "local:macos",
        "--window-title",
        "Draft",
        "--focus-policy",
        "never",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "close-window");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "target_selector": {
                "WindowTitle": "Draft"
            },
            "focus_policy": "Never"
        })
    );
}

#[test]
fn launch_app_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "launch-app",
        "--bundle-id-or-name",
        "Calculator",
        "--verify",
        "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[test]
fn close_window_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "close-window",
        "--window-id",
        "42",
        "--verify",
        "window-state",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[tokio::test]
async fn minimize_window_command_maps_app_target_selector_to_tool_input() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "minimize-window", "--app", "TextEdit"])
            .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "minimize-window");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "App": "TextEdit"
            },
            "focus_policy": "Auto"
        })
    );
}

#[tokio::test]
async fn minimize_window_command_only_accepts_window_state_verification() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "minimize-window",
        "--window-id",
        "42",
        "--verify",
        "window-state",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "minimize-window");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "WindowId": 42
            },
            "focus_policy": "Auto",
            "verifications": ["WindowState"]
        })
    );

    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "minimize-window",
        "--window-id",
        "42",
        "--verify",
        "focus",
    ])
    .unwrap_err();
    assert!(error.to_string().contains("invalid value 'focus'"));
}

#[tokio::test]
async fn maximize_window_command_maps_pid_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "maximize-window",
        "--target",
        "local:macos",
        "--pid",
        "101",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "maximize-window");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "target_selector": {
                "Pid": 101
            },
            "focus_policy": "Auto"
        })
    );
}

#[test]
fn maximize_window_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "maximize-window",
        "--window-id",
        "42",
        "--verify",
        "window-state",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[tokio::test]
async fn move_window_command_maps_coordinates_and_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "move-window",
        "--target",
        "local:macos",
        "--window-id",
        "42",
        "--x",
        "120",
        "--y",
        "240",
        "--focus-policy",
        "never",
        "--verify",
        "geometry",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "move-window");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "target_selector": {
                "WindowId": 42
            },
            "focus_policy": "Never",
            "verifications": ["Geometry"],
            "x": 120.0,
            "y": 240.0
        })
    );
}

#[tokio::test]
async fn resize_window_command_maps_size_and_app_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "resize-window",
        "--app",
        "TextEdit",
        "--width",
        "640",
        "--height",
        "480",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "resize-window");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "App": "TextEdit"
            },
            "focus_policy": "Auto",
            "width": 640.0,
            "height": 480.0
        })
    );
}

#[tokio::test]
async fn set_window_bounds_command_maps_rect_and_pid_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "set-window-bounds",
        "--target",
        "local:macos",
        "--pid",
        "101",
        "--x",
        "80",
        "--y",
        "120",
        "--width",
        "900",
        "--height",
        "700",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "set-window-bounds");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "target_selector": {
                "Pid": 101
            },
            "focus_policy": "Auto",
            "x": 80.0,
            "y": 120.0,
            "width": 900.0,
            "height": 700.0
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
        "--verify",
        "focus",
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
            },
            "verifications": ["Focus"]
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
async fn cli_run_renders_close_window_detail_for_non_json_output() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "close-window", "--window-id", "42"])
            .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 10,
                "detail": "closed window 42"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "closed window 42");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "close-window");
    assert_eq!(
        recorded[0].1,
        json!({
            "target_selector": {
                "WindowId": 42
            },
            "focus_policy": "Auto"
        })
    );
}

#[tokio::test]
async fn cli_run_renders_move_window_detail_for_non_json_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "move-window",
        "--window-id",
        "42",
        "--x",
        "120",
        "--y",
        "240",
    ])
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 10,
                "detail": "moved window 42 to x=120 y=240 width=640 height=480"
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(
        rendered,
        "moved window 42 to x=120 y=240 width=640 height=480"
    );

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "move-window");
    assert_eq!(
        recorded[0].1,
        json!({
            "target_selector": {
                "WindowId": 42
            },
            "focus_policy": "Auto",
            "x": 120.0,
            "y": 240.0
        })
    );
}

#[tokio::test]
async fn cli_run_renders_move_window_from_structured_outcome_when_detail_is_missing() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "move-window",
        "--window-id",
        "42",
        "--x",
        "120",
        "--y",
        "240",
    ])
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 10,
                "target_window": {
                    "id": 42,
                    "title": "Draft",
                    "app_name": "TextEdit",
                    "bounds": {
                        "x": 120.0,
                        "y": 240.0,
                        "width": 640.0,
                        "height": 480.0
                    },
                    "is_focused": true,
                    "is_minimized": false
                },
                "side_effects": [
                    {
                        "kind": "MoveWindow",
                        "data": {
                            "bounds": {
                                "x": 120.0,
                                "y": 240.0,
                                "width": 640.0,
                                "height": 480.0
                            }
                        }
                    }
                ]
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(
        rendered,
        "moved window 42 to x=120 y=240 width=640 height=480"
    );
}

#[tokio::test]
async fn cli_run_renders_focus_window_from_structured_outcome_when_detail_is_missing() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "focus-window", "--window-id", "42"])
            .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "outcome": {
                "success": true,
                "duration_ms": 5,
                "target_window": {
                    "id": 42,
                    "title": "Draft",
                    "app_name": "TextEdit",
                    "bounds": {
                        "x": 40.0,
                        "y": 60.0,
                        "width": 640.0,
                        "height": 480.0
                    },
                    "is_focused": true,
                    "is_minimized": false
                },
                "side_effects": [
                    {
                        "kind": "FocusWindow"
                    }
                ]
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "focused window 42");
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
