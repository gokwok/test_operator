#[path = "../src/main.rs"]
mod cli_main;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use serde_json::{json, Value};

#[test]
fn observe_frontmost_command_defaults_capture_to_all() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "observe", "frontmost", "--json"])
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
            "include_screenshot": true,
            "include_elements": true
        })
    );
}

#[test]
fn observe_window_command_maps_surface_and_capture_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "observe",
        "window",
        "--window-id",
        "42",
        "--capture",
        "elements",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "observe");
    assert_eq!(
        invocation.input,
        json!({
            "surface": {
                "kind": {
                    "Window": {
                        "id": 42
                    }
                }
            },
            "include_screenshot": false,
            "include_elements": true
        })
    );
}

#[test]
fn observe_fullscreen_command_maps_display_id_and_capture_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "observe",
        "fullscreen",
        "--display-id",
        "2",
        "--capture",
        "screenshot",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "observe");
    assert_eq!(
        invocation.input,
        json!({
            "surface": {
                "kind": {
                    "Fullscreen": {
                        "display_id": 2
                    }
                }
            },
            "include_screenshot": true,
            "include_elements": false
        })
    );
}

#[test]
fn observe_region_command_maps_rect_and_capture_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "observe",
        "region",
        "--x",
        "10",
        "--y",
        "20",
        "--width",
        "300",
        "--height",
        "200",
        "--capture",
        "none",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "observe");
    assert_eq!(
        invocation.input,
        json!({
            "surface": {
                "kind": {
                    "Region": {
                        "rect": {
                            "x": 10.0,
                            "y": 20.0,
                            "width": 300.0,
                            "height": 200.0
                        }
                    }
                }
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
fn mcp_help_lists_serve_subcommand() {
    let help = command_help(["operator", "mcp", "--help"]);
    assert!(help.contains("Run MCP stdio server commands"));
    assert!(help.contains("Usage operator mcp [OPTIONS] <COMMAND>"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator mcp serve"));
    assert!(help.contains("Use 'operator mcp <command> --help' for detailed usage."));
}

#[test]
fn mcp_serve_command_maps_to_mcp_execution_mode() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "mcp", "serve"]).unwrap();

    let execution = cli.into_execution().unwrap();
    assert!(matches!(execution, cli_main::args::CliExecution::McpServe));
}

#[test]
fn permissions_help_shows_examples() {
    let help = command_help(["operator", "permissions", "--help"]);
    assert!(help.contains("Check automation permissions and runtime readiness"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Override the runtime timeout for this command"));
    assert!(help.contains("Examples\n  operator permissions\n  operator --json permissions"));
}

#[test]
fn capabilities_help_shows_examples() {
    let help = command_help(["operator", "capabilities", "--help"]);
    assert!(help.contains("Show supported surfaces, queries, and actions for the active target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator capabilities\n  operator capabilities --json"));
}

#[test]
fn list_help_lists_query_subcommands_and_examples() {
    let help = command_help(["operator", "list", "--help"]);
    assert!(help.contains("List running apps or windows"));
    assert!(help.contains("apps"));
    assert!(help.contains("List running applications"));
    assert!(help.contains("windows"));
    assert!(help.contains("List windows, optionally filtered by app"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Use 'operator list <command> --help' for detailed usage."));
}

#[test]
fn focus_help_shows_examples() {
    let help = command_help(["operator", "focus", "--help"]);
    assert!(help.contains("Show the currently focused app, window, and element"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Examples\n  operator focus\n  operator --json focus"));
}

#[test]
fn root_help_groups_commands_by_domain() {
    let help = command_help(["operator", "--help"]);
    assert!(help.contains("Usage operator [OPTIONS] [COMMAND]"));
    assert!(help.contains("Operator - Turn any desktop app into an API, from CLI to AI"));
    assert!(!help.contains("Tip:\n  Start with operator observe --help"));
    assert!(help.contains("permissions   Check automation permissions and runtime readiness"));
    assert!(help.contains("snapshot      Read stored snapshots by ID"));
    assert!(help.contains("focus         Show the currently focused app, window, and element"));
    assert!(help.contains(
        "input         Pointer and keyboard actions against locators or target windows/apps"
    ));
    assert!(help.contains("mcp           Run MCP stdio server commands"));
    assert!(
        help.contains("A2A\n  Not yet implemented. Reserved for future agent interface commands.")
    );
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Examples\n  operator observe frontmost"));
    assert!(help.contains(
        "Use 'operator <group> --help' or 'operator <group> <command> --help' for detailed usage."
    ));
}

#[test]
fn observe_help_lists_surface_subcommands() {
    let help = command_help(["operator", "observe", "--help"]);
    assert!(
        help.contains("Create snapshots from frontmost, window, region, or fullscreen surfaces")
    );
    assert!(help.contains("frontmost"));
    assert!(help.contains("Create a snapshot from the frontmost surface"));
    assert!(help.contains("window"));
    assert!(help.contains("Create a snapshot from a specific window"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Use 'operator observe <command> --help' for detailed usage."));
}

#[test]
fn snapshot_help_lists_get_subcommand() {
    let help = command_help(["operator", "snapshot", "--help"]);
    assert!(help.contains("Read stored snapshots by ID"));
    assert!(help.contains("get"));
    assert!(help.contains("Read a stored snapshot by ID"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Use 'operator snapshot <command> --help' for detailed usage."));
}

#[test]
fn artifact_help_lists_get_subcommand() {
    let help = command_help(["operator", "artifact", "--help"]);
    assert!(help.contains("Resolve stored capture artifacts by ID"));
    assert!(help.contains("get"));
    assert!(help.contains("Resolve a stored capture artifact by ID"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Use 'operator artifact <command> --help' for detailed usage."));
}

#[test]
fn input_help_lists_pointer_and_keyboard_subcommands() {
    let help = command_help(["operator", "input", "--help"]);
    assert!(help.starts_with(
        "Usage operator input [OPTIONS] <COMMAND>\n\nPointer and keyboard actions against locators or target windows/apps"
    ));
    assert!(help.contains("Pointer and keyboard actions against locators or target windows/apps"));
    assert!(help.contains("click"));
    assert!(help.contains("Click a locator, coordinate, or target"));
    assert!(help.contains("Use 'operator input <command> --help' for detailed usage."));
}

#[test]
fn input_type_help_shows_positional_text_and_after_key() {
    let help = command_help(["operator", "input", "type", "--help"]);
    assert!(help.contains("Usage operator input type [OPTIONS] <TEXT>"));
    assert!(help.contains("--after-key <AFTER_KEYS>"));
    assert!(help.contains("--focus <FOCUS>"));
    assert!(help.contains("Examples\n  operator input type \"hello operator\""));
}

#[test]
fn app_help_lists_lifecycle_subcommands() {
    let help = command_help(["operator", "app", "--help"]);
    assert!(help.contains("Launch, switch, hide, quit, and relaunch applications"));
    assert!(help.contains("launch"));
    assert!(help.contains("Launch an application by bundle identifier or name"));
    assert!(help.contains("Use 'operator app <command> --help' for detailed usage."));
}

#[test]
fn window_help_lists_window_management_subcommands() {
    let help = command_help(["operator", "window", "--help"]);
    assert!(help.contains("Focus, close, resize, or move application windows"));
    assert!(help.contains("set-bounds"));
    assert!(help.contains("Set the full bounds of a specific window"));
    assert!(help.contains("Use 'operator window <command> --help' for detailed usage."));
}

#[test]
fn root_help_uses_highlight_and_muted_tip_styles() {
    let help = styled_command_help(["operator", "--help"]);

    assert!(help.contains("\u{1b}[1;38;5;214mUsage\u{1b}[0m"));
    assert!(help.contains("\u{1b}[1;38;5;255moperator\u{1b}[0m"));
    assert!(help.contains("\u{1b}[38;5;245mUse 'operator <group> --help'"));
    assert!(!help.contains("\u{1b}[38;5;245mTip"));
}

#[test]
fn window_resize_help_shows_focus_and_verify_flags() {
    let help = command_help(["operator", "window", "resize", "--help"]);
    assert!(
        help.contains("Usage operator window resize [OPTIONS] --width <WIDTH> --height <HEIGHT>")
    );
    assert!(help.contains("--focus <FOCUS>"));
    assert!(help.contains("--verify <VERIFICATIONS>"));
    assert!(help.contains("Examples\n  operator window resize --window-id 42"));
}

#[test]
fn observe_window_help_snapshot_is_stable() {
    let help = command_help(["operator", "observe", "window", "--help"]);
    assert!(help.contains("Capture a specific window"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Override the runtime timeout for this command"));
}

#[test]
fn snapshot_get_help_snapshot_is_stable() {
    let help = command_help(["operator", "snapshot", "get", "--help"]);
    assert!(help.contains("Read a stored snapshot by ID"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator snapshot get s_123"));
}

#[test]
fn artifact_get_help_snapshot_is_stable() {
    let help = command_help(["operator", "artifact", "get", "--help"]);
    assert!(help.contains("Resolve a stored capture artifact by ID"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator artifact get capture-1.png"));
}

#[test]
fn input_click_help_snapshot_is_stable() {
    let help = command_help(["operator", "input", "click", "--help"]);
    assert!(help.starts_with("Usage operator input click"));
    assert!(help.contains(
        "Usage operator input click [OPTIONS]\n\nClick a locator, coordinate, or target"
    ));
    assert!(help.contains("Click a locator, coordinate, or target"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("--mode <MODE>"));
}

#[test]
fn app_launch_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "launch", "--help"]);
    assert!(help.contains("Launch an application by bundle identifier or name"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator app launch Calculator"));
}

#[test]
fn app_switch_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "switch", "--help"]);
    assert!(help.contains("Bring an application to the foreground"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("--verify <VERIFICATIONS>"));
}

#[test]
fn window_focus_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "focus", "--help"]);
    assert!(help.contains("Focus a specific window"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("--verify <VERIFICATIONS>"));
}

#[test]
fn window_resize_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "resize", "--help"]);
    assert!(help.contains("Resize a specific window"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("--focus <FOCUS>"));
    assert!(help.contains("--verify <VERIFICATIONS>"));
}

#[test]
fn mcp_serve_help_snapshot_is_stable() {
    let help = command_help(["operator", "mcp", "serve", "--help"]);
    assert!(help.contains("Run the MCP stdio server"));
    assert!(help.contains("Select the runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator mcp serve"));
}

#[test]
fn snapshot_get_command_maps_positional_snapshot_id_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "snapshot",
        "get",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "s_123",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "snapshot-get");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "snapshot_id": "s_123"
        })
    );
}

#[test]
fn artifact_get_command_maps_positional_artifact_id_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "artifact",
        "get",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
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
fn legacy_flat_commands_show_grouped_replacement_hints() {
    let cases: [(&[&str], &str, &str); 27] = [
        (
            &["operator", "snapshot-get", "s_123"],
            "snapshot-get",
            "operator snapshot get",
        ),
        (
            &["operator", "artifact-get", "capture-1.png"],
            "artifact-get",
            "operator artifact get",
        ),
        (&["operator", "get-focus"], "get-focus", "operator focus"),
        (
            &["operator", "list-apps"],
            "list-apps",
            "operator list apps",
        ),
        (
            &["operator", "list-windows"],
            "list-windows",
            "operator list windows",
        ),
        (
            &["operator", "permissions-status"],
            "permissions-status",
            "operator permissions",
        ),
        (&["operator", "click"], "click", "operator input click"),
        (&["operator", "move"], "move", "operator input move"),
        (&["operator", "type"], "type", "operator input type"),
        (&["operator", "press"], "press", "operator input press"),
        (&["operator", "hotkey"], "hotkey", "operator input hotkey"),
        (&["operator", "scroll"], "scroll", "operator input scroll"),
        (&["operator", "drag"], "drag", "operator input drag"),
        (&["operator", "swipe"], "swipe", "operator input swipe"),
        (
            &["operator", "launch-app"],
            "launch-app",
            "operator app launch",
        ),
        (
            &["operator", "switch-app"],
            "switch-app",
            "operator app switch",
        ),
        (&["operator", "quit-app"], "quit-app", "operator app quit"),
        (
            &["operator", "relaunch-app"],
            "relaunch-app",
            "operator app relaunch",
        ),
        (&["operator", "hide-app"], "hide-app", "operator app hide"),
        (
            &["operator", "unhide-app"],
            "unhide-app",
            "operator app unhide",
        ),
        (
            &["operator", "focus-window"],
            "focus-window",
            "operator window focus",
        ),
        (
            &["operator", "close-window"],
            "close-window",
            "operator window close",
        ),
        (
            &["operator", "minimize-window"],
            "minimize-window",
            "operator window minimize",
        ),
        (
            &["operator", "maximize-window"],
            "maximize-window",
            "operator window maximize",
        ),
        (
            &["operator", "move-window"],
            "move-window",
            "operator window move",
        ),
        (
            &["operator", "resize-window"],
            "resize-window",
            "operator window resize",
        ),
        (
            &["operator", "set-window-bounds"],
            "set-window-bounds",
            "operator window set-bounds",
        ),
    ];

    for (args, legacy, replacement) in cases {
        assert_legacy_command_migration(args, legacy, replacement);
    }
}

#[test]
fn legacy_flat_command_detection_skips_root_global_flags() {
    assert_legacy_command_migration(
        &["operator", "--json", "--target", "local:macos", "click"],
        "click",
        "operator input click",
    );
}

#[tokio::test]
async fn input_click_command_maps_locator_target_focus_and_verification() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "click",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--mode",
        "double",
        "--snapshot",
        "s_123",
        "--element",
        "e_45",
        "--window-title",
        "Project Notes",
        "--focus",
        "never",
        "--verify",
        "focus",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "click");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "mode": "Double",
            "locator": {
                "SnapshotElement": {
                    "snapshot": "s_123",
                    "element": "e_45"
                }
            },
            "target_selector": {
                "WindowTitle": "Project Notes"
            },
            "focus_policy": "Never",
            "verifications": ["Focus"]
        })
    );
}

#[test]
fn input_click_command_rejects_conflicting_locator_variants() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "input", "click", "--text", "Save", "--x", "24", "--y", "48",
    ])
    .unwrap();

    let error = cli.into_invocation().unwrap_err();
    assert_eq!(error, "locator flags are mutually exclusive");
}

#[tokio::test]
async fn window_focus_command_maps_window_id_and_verification_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "focus",
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
async fn window_close_command_maps_target_selector_and_focus_policy() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "close",
        "--target",
        "local:macos",
        "--window-title",
        "Draft",
        "--focus",
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
fn window_close_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "close",
        "--window-id",
        "42",
        "--verify",
        "window-state",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[tokio::test]
async fn window_minimize_command_maps_selector_and_window_state_verification() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "minimize",
        "--app",
        "TextEdit",
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
                "App": "TextEdit"
            },
            "focus_policy": "Auto",
            "verifications": ["WindowState"]
        })
    );
}

#[test]
fn window_minimize_command_only_accepts_window_state_verification() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "minimize",
        "--window-id",
        "42",
        "--verify",
        "focus",
    ])
    .unwrap_err();
    assert!(error.to_string().contains("invalid value 'focus'"));
}

#[tokio::test]
async fn window_maximize_command_maps_pid_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "maximize",
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
fn window_maximize_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "maximize",
        "--window-id",
        "42",
        "--verify",
        "window-state",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[tokio::test]
async fn window_move_command_maps_coordinates_selector_focus_and_verification() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "move",
        "--target",
        "local:macos",
        "--window-id",
        "42",
        "--x",
        "120",
        "--y",
        "240",
        "--focus",
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
async fn window_resize_command_maps_size_and_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "window", "resize", "--app", "TextEdit", "--width", "640", "--height", "480",
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
async fn window_set_bounds_command_maps_rect_and_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "window",
        "set-bounds",
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

#[test]
fn app_launch_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "launch",
        "Calculator",
        "--verify",
        "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[tokio::test]
async fn app_launch_command_maps_positional_bundle_id_or_name_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "launch",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "Calculator",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "launch-app");
    assert_eq!(
        invocation.input,
        json!({
            "target": "local:macos",
            "timeout_ms": 250,
            "bundle_id_or_name": "Calculator"
        })
    );
}

#[tokio::test]
async fn app_switch_command_maps_app_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "switch",
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
async fn app_quit_command_maps_pid_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "quit",
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
async fn app_relaunch_command_maps_window_title_target_selector_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "relaunch",
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
async fn app_hide_command_maps_window_index_target_selector_to_tool_input() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "app", "hide", "--window-index", "1"])
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
async fn app_unhide_command_maps_window_id_target_selector_to_tool_input() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "app", "unhide", "--window-id", "42"])
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
async fn input_type_command_maps_app_target_selector_with_default_focus_policy() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "type",
        "--target",
        "local:macos",
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
async fn input_scroll_command_maps_locator_and_deltas_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
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

#[test]
fn input_scroll_command_rejects_incomplete_snapshot_locator() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "scroll",
        "--delta-x",
        "0",
        "--delta-y",
        "-120",
        "--snapshot",
        "s_123",
    ])
    .unwrap();

    let error = cli.into_invocation().unwrap_err();
    assert_eq!(error, "--element is required when --snapshot is provided");
}

#[tokio::test]
async fn input_move_command_maps_coordinate_locator_and_verification() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
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
async fn input_drag_command_maps_motion_options_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
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
async fn input_swipe_command_maps_motion_options_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
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
async fn input_hotkey_command_maps_positional_keys_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "hotkey",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "command",
        "shift",
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
async fn input_press_command_maps_positional_key_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "press",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
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

#[test]
fn input_type_command_rejects_legacy_trailing_key_flag() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "type",
        "hello world",
        "--trailing-key",
        "return",
    ])
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("unexpected argument '--trailing-key'"));
}

#[tokio::test]
async fn input_type_command_maps_positional_text_after_keys_and_locator_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "input",
        "type",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "hello world",
        "--clear-before",
        "--delay-ms",
        "25",
        "--after-key",
        "return",
        "--after-key",
        "tab",
        "--text",
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
        "input",
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
        "app",
        "launch",
        "--json",
        "--target",
        "local:macos",
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
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "app", "switch", "--app", "TextEdit"])
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
        cli_main::args::Cli::try_parse_from(["operator", "window", "close", "--window-id", "42"])
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
        "window",
        "move",
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
        "window",
        "move",
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
        cli_main::args::Cli::try_parse_from(["operator", "window", "focus", "--window-id", "42"])
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
        cli_main::args::Cli::try_parse_from(["operator", "input", "press", "down", "--count", "3"])
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
    let cli = cli_main::args::Cli::try_parse_from(["operator", "artifact", "get", "capture-1.png"])
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
        "operator", "input", "swipe", "--from-x", "10", "--from-y", "20", "--to-x", "100",
        "--to-y", "20",
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

fn command_help<const N: usize>(args: [&str; N]) -> String {
    strip_ansi(&styled_command_help(args))
}

fn styled_command_help<const N: usize>(args: [&str; N]) -> String {
    let argv = args
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();

    if let Some(help) = cli_main::args::custom_help(&argv) {
        return help;
    }

    cli_main::args::Cli::try_parse_from(argv)
        .unwrap_err()
        .to_string()
}

fn strip_ansi(input: &str) -> String {
    let mut stripped = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            let _ = chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }

        stripped.push(ch);
    }

    stripped
}

fn assert_legacy_command_migration(args: &[&str], legacy: &str, replacement: &str) {
    let error = cli_main::args::Cli::try_parse_from(args.iter().copied()).unwrap_err();
    let message = error.to_string();

    assert!(
        message.contains(legacy),
        "missing legacy command in `{message}`"
    );
    assert!(
        message.contains(replacement),
        "missing replacement command in `{message}`"
    );
}
