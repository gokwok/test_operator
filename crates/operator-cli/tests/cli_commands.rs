#[path = "../src/main.rs"]
mod cli_main;

use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    num::NonZeroU32,
    pin::Pin,
    sync::{Arc, Mutex},
};

use operator_agent::AgentRunResult;
use operator_bootstrap::runtime_config_path;
use operator_core::{DriverConfig, OperatorError, SessionId, TargetId};
use operator_runtime::{NamedTargetConfig, RuntimeBuilder, RuntimeConfig, ToolRegistry};
use operator_testkit::InMemorySnapshotStore;
use serde_json::{json, Value};
use tempfile::tempdir;

#[test]
fn capture_frontmost_command_defaults_to_screenshot_only() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "capture", "frontmost", "--json"])
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
            "include_elements": false
        })
    );
}

#[test]
fn elements_frontmost_command_defaults_to_tree_only() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "elements", "frontmost"]).unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "observe");
    assert_eq!(
        invocation.input,
        json!({
            "surface": {
                "kind": "Frontmost"
            },
            "include_screenshot": false,
            "include_elements": true
        })
    );
}

#[test]
fn elements_window_command_maps_surface_and_tree_only_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "elements",
        "window",
        "--window-id",
        "42",
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
fn capture_window_command_accepts_synthetic_window_ids() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "capture",
        "window",
        "--window-id",
        "9223372036854775850",
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
                        "id": 9_223_372_036_854_775_850u64
                    }
                }
            },
            "include_screenshot": true,
            "include_elements": false
        })
    );
}

#[test]
fn elements_window_command_accepts_synthetic_window_ids() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "elements",
        "window",
        "--window-id",
        "9223372036854775850",
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
                        "id": 9_223_372_036_854_775_850u64
                    }
                }
            },
            "include_screenshot": false,
            "include_elements": true
        })
    );
}

#[test]
fn capture_region_command_maps_rect_and_screenshot_only_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "capture", "region", "--x", "10", "--y", "20", "--width", "300", "--height",
        "200",
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
            "include_screenshot": true,
            "include_elements": false
        })
    );
}

#[test]
fn capture_fullscreen_command_maps_display_id_and_screenshot_only_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "capture",
        "fullscreen",
        "--display-id",
        "2",
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
fn elements_region_command_maps_rect_and_tree_only_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "elements", "region", "--x", "10", "--y", "20", "--width", "300", "--height",
        "200",
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
            "include_elements": true
        })
    );
}

#[test]
fn elements_fullscreen_command_maps_display_id_and_tree_only_profile() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "elements",
        "fullscreen",
        "--display-id",
        "2",
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
            "include_screenshot": false,
            "include_elements": true
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
fn show_command_maps_common_flags_to_internal_tool() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "show",
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
fn app_list_command_maps_to_list_apps_tool() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "app", "list"]).unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-apps");
    assert_eq!(invocation.input, json!({ "mode": "running" }));
}

#[test]
fn app_list_all_command_maps_to_list_apps_tool() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "app", "list", "--all"]).unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-apps");
    assert_eq!(invocation.input, json!({ "mode": "all" }));
}

#[test]
fn app_list_name_filter_maps_to_list_apps_tool() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "app", "list", "--name", "Cod"]).unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-apps");
    assert_eq!(invocation.input, json!({ "mode": "all", "name": "Cod" }));
}

#[test]
fn app_list_bundle_filter_maps_to_list_apps_tool() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "list",
        "--bundle",
        "com.apple.TextEdit",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-apps");
    assert_eq!(
        invocation.input,
        json!({ "mode": "all", "bundle": "com.apple.TextEdit" })
    );
}

#[test]
fn app_list_running_filter_keeps_explicit_running_mode() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "app",
        "list",
        "--running",
        "--bundle",
        "com.apple.TextEdit",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "list-apps");
    assert_eq!(
        invocation.input,
        json!({ "mode": "running", "bundle": "com.apple.TextEdit" })
    );
}

#[test]
fn app_list_rejects_conflicting_modes() {
    let error =
        cli_main::args::Cli::try_parse_from(["operator", "app", "list", "--running", "--all"])
            .unwrap_err();

    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn window_list_command_maps_to_list_windows_tool() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "window", "list", "--app", "TextEdit"])
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
fn window_list_requires_app_in_cli_contract() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "window", "list"]).unwrap();

    let error = cli.into_invocation().unwrap_err();
    assert_eq!(
        error,
        "window list requires --app <NAME>; unfiltered window enumeration is no longer supported by the CLI"
    );
}

#[test]
fn mcp_help_lists_serve_subcommand() {
    let help = command_help(["operator", "mcp", "--help"]);
    assert!(help.contains("Run the Operator MCP server"));
    assert!(help.contains("serve"));
    assert!(help.contains("Start the MCP stdio server"));
    assert!(help.contains("Usage operator mcp [OPTIONS] <COMMAND>"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Examples\n  operator mcp serve"));
    assert!(!help.contains("Use 'operator mcp <command> --help' for detailed usage."));
}

#[test]
fn mcp_serve_command_maps_to_mcp_execution_mode() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "mcp", "serve"]).unwrap();

    let execution = cli.into_execution().unwrap();
    assert!(matches!(execution, cli_main::args::CliExecution::McpServe));
}

#[test]
fn agent_help_shows_first_phase_flags_and_examples() {
    let help = command_help(["operator", "agent", "--help"]);
    assert!(help.contains("Execute a natural-language task against the active target."));
    assert!(help.contains("observes the screen, plans actions, and drives the UI autonomously"));
    assert!(help.contains("Usage operator agent [OPTIONS] <TASK>"));
    assert!(help.contains("Arguments\n  <TASK>"));
    assert!(help.contains("--model <MODEL>"));
    assert!(help.contains("--max-steps <N>"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("--target <TARGET>"));
    assert!(help.contains("--timeout-ms <TIMEOUT_MS>"));
    assert!(help.contains("--json"));
    assert!(help.contains("Examples\n  operator agent \"Open Notes and type hello\""));
    assert!(help.contains(
        "operator agent \"Find the largest file in Downloads and move it to the Trash\""
    ));
    assert!(help.contains(
        "operator agent --model doubao-seed --max-steps 10 \"Summarize the frontmost window\""
    ));
}

#[test]
fn agent_command_maps_task_and_first_phase_flags_to_agent_execution() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "agent",
        "--json",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--model",
        "doubao-seed",
        "--max-steps",
        "8",
        "Summarize the frontmost window",
    ])
    .unwrap();

    assert!(cli.prefers_json());

    let execution = cli.into_execution().unwrap();
    let cli_main::args::CliExecution::Agent(command) = execution else {
        panic!("agent command should map to agent execution");
    };

    assert_eq!(command.task, "Summarize the frontmost window");
    assert_eq!(command.model.as_deref(), Some("doubao-seed"));
    assert_eq!(command.max_steps, Some(NonZeroU32::new(8).unwrap()));
    assert_eq!(command.target.as_deref(), Some("local:macos"));
    assert_eq!(command.timeout_ms, Some(250));
    assert!(command.json_output);
}

#[test]
fn agent_runtime_config_loads_named_targets_from_operator_home_and_applies_flag_overrides() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        runtime_config_path(temp.path()),
        r#"
[runtime]
default_target = "windows-lab"
default_timeout_ms = 500

[targets.windows-lab]
platform = "windows"
driver = "windows.remote"

[targets.windows-lab.driver_config]
endpoint = "wss://windows-lab.internal"
"#,
    )
    .expect("write config");

    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "agent",
        "--target",
        "harmony-phone",
        "--timeout-ms",
        "250",
        "Summarize the frontmost window",
    ])
    .unwrap();

    let cli_main::args::CliExecution::Agent(command) = cli.into_execution().unwrap() else {
        panic!("agent command should map to agent execution");
    };
    let config = cli_main::runtime_config_for_home(&command, temp.path()).expect("runtime config");

    assert_eq!(config.default_target, TargetId("harmony-phone".into()));
    assert_eq!(config.default_timeout_ms, 250);
    let windows_target = config.targets.get("windows-lab").expect("windows target");
    assert_eq!(windows_target.platform, "windows");
    assert_eq!(windows_target.driver, "windows.remote");
    assert_eq!(
        windows_target.driver_config.get("endpoint"),
        Some(&json!("wss://windows-lab.internal"))
    );
}

#[test]
fn permissions_help_shows_examples() {
    let help = command_help(["operator", "permissions", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator permissions [OPTIONS]",
        "Check automation permissions and runtime readiness",
        &["operator permissions", "operator --json permissions"],
    );
}

#[test]
fn capabilities_help_shows_examples() {
    let help = command_help(["operator", "capabilities", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator capabilities [OPTIONS]",
        "Show supported surfaces, queries, and actions for the active target",
        &["operator capabilities", "operator --json capabilities"],
    );
}

#[test]
fn show_help_shows_examples() {
    let help = command_help(["operator", "show", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator show [OPTIONS]",
        "Show the currently focused app, window, and element",
        &["operator show", "operator --json show"],
    );
}

#[test]
fn root_help_groups_commands_by_domain() {
    let help = command_help(["operator", "--help"]);
    assert!(help.contains("Usage operator [OPTIONS] <COMMAND>"));
    assert!(help.contains("Operator - Turn any desktop app into an API, from CLI to AI"));
    assert!(!help.contains("Tip:\n  Start with operator observe --help"));
    assert!(help.contains("Core"));
    assert!(help.contains("Observe"));
    assert!(help.contains("Interact"));
    assert!(help.contains("System"));
    assert!(help.contains("Integration"));
    assert!(help.contains("AI"));
    assert!(!help.contains("\nQuery\n"));
    assert!(!help.contains("\nAction\n"));
    assert!(!help.contains("\nMCP\n"));
    assert!(!help.contains("\nAgent\n"));
    assert!(help.contains("permissions"));
    assert!(help.contains("Check automation permissions and runtime readiness"));
    assert!(help.contains("snapshot"));
    assert!(help.contains("Read a stored snapshot by ID"));
    assert!(help.contains("artifact"));
    assert!(help.contains("Read a stored capture artifact by ID"));
    assert!(help.contains("capture"));
    assert!(help.contains("Take a screenshot of a surface"));
    assert!(help.contains("elements"));
    assert!(help.contains("Query the accessibility element tree for a surface"));
    assert!(help.contains("show"));
    assert!(help.contains("Show the currently focused app, window, and element"));
    assert!(help.contains("click"));
    assert!(help.contains("Click a locator, coordinates, or target"));
    assert!(help.contains("paste"));
    assert!(help.contains("Clipboard-aware paste [planned]"));
    assert!(help.contains("window"));
    assert!(help.contains("Manage application windows"));
    assert!(help.contains("clipboard"));
    assert!(help.contains("Read/write the clipboard [planned]"));
    assert!(help.contains("open"));
    assert!(help.contains("Open a URL or file with its default application [planned]"));
    assert!(!help.contains("operator list windows"));
    assert!(help.contains("mcp"));
    assert!(help.contains("Run the Operator MCP server"));
    assert!(help.contains("agent"));
    assert!(help.contains("Execute a natural-language task against a target"));
    assert!(!help.contains("Not yet implemented. Reserved for future agent interface commands."));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Examples\n  operator capture frontmost"));
    assert!(!help.contains("operator window list"));
    assert!(help.contains("Use 'operator <command> --help' for detailed usage."));
}

#[test]
fn capture_help_lists_surface_subcommands() {
    let help = command_help(["operator", "capture", "--help"]);
    assert!(help.contains("Take a screenshot of a surface"));
    assert!(help.contains("frontmost"));
    assert!(help.contains("Take a screenshot of the frontmost app surface"));
    assert!(help.contains("window"));
    assert!(help.contains("Take a screenshot of a specific window"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Use 'operator capture <surface> --help' for detailed usage."));
}

#[test]
fn elements_help_lists_surface_subcommands() {
    let help = command_help(["operator", "elements", "--help"]);
    assert!(help.contains("Query the accessibility element tree for a surface"));
    assert!(help.contains("frontmost"));
    assert!(help.contains("Query the accessibility element tree for the frontmost app surface"));
    assert!(help.contains("window"));
    assert!(help.contains("Query the accessibility element tree for a specific window"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Use 'operator elements <surface> --help' for detailed usage."));
}

#[test]
fn snapshot_help_shows_direct_id_usage() {
    let help = command_help(["operator", "snapshot", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator snapshot [OPTIONS] <SNAPSHOT-ID>",
        "Read a stored snapshot by ID",
        &[
            "operator snapshot s_abc123",
            "operator --json snapshot s_abc123",
        ],
    );
    assert!(help.contains("Arguments"));
    assert!(help.contains("<SNAPSHOT-ID>"));
}

#[test]
fn artifact_help_shows_direct_id_usage() {
    let help = command_help(["operator", "artifact", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator artifact [OPTIONS] <ARTIFACT-ID>",
        "Read a stored capture artifact by ID",
        &[
            "operator artifact capture-1.png",
            "operator --json artifact capture-1.png",
        ],
    );
    assert!(help.contains("Arguments"));
    assert!(help.contains("<ARTIFACT-ID>"));
}

#[test]
fn type_help_shows_positional_text_and_after_key() {
    let help = command_help(["operator", "type", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator type [OPTIONS] <TEXT>",
        "Type text into the focused or resolved target",
        &[
            "operator type \"hello world\"",
            "operator type \"search query\" --text \"Search...\" --after-key return",
        ],
    );
    assert!(help.contains("Arguments"));
    assert!(help.contains("Locator (pick one group)"));
    assert!(help.contains("Target (optional, defaults to frontmost)"));
    assert!(help.contains("--after-key return|tab|escape|delete"));
    assert!(help.contains("--focus auto|never"));
}

#[test]
fn app_help_lists_lifecycle_subcommands() {
    let help = command_help(["operator", "app", "--help"]);
    assert!(help.contains("Manage application lifecycle"));
    assert!(help.contains("list"));
    assert!(help.contains("List operable applications"));
    assert!(help.contains("launch"));
    assert!(help.contains("Launch an application"));
    assert!(help.contains("Use 'operator app <command> --help' for detailed usage."));
}

#[test]
fn window_help_lists_window_management_subcommands() {
    let help = command_help(["operator", "window", "--help"]);
    assert!(help.contains("Manage application windows"));
    assert!(help.contains("list"));
    assert!(help.contains("List application windows"));
    assert!(help.contains("set-bounds"));
    assert!(help.contains("Set the full position and size of a window in one operation"));
    assert!(help.contains("Use 'operator window <command> --help' for detailed usage."));
}

#[test]
fn root_help_uses_highlight_and_muted_tip_styles() {
    let help = styled_command_help(["operator", "--help"]);

    assert!(help.contains("\u{1b}[1;38;5;214mUsage\u{1b}[0m"));
    assert!(help.contains("\u{1b}[1;38;5;255moperator [OPTIONS] <COMMAND>\u{1b}[0m"));
    assert!(help.contains("\u{1b}[38;5;245mUse 'operator <command> --help'"));
    assert!(!help.contains("\u{1b}[38;5;245mTip"));
}

#[test]
fn root_help_keeps_global_runtime_flag_descriptions_on_the_same_line() {
    let help = command_help(["operator", "--help"]);
    let timeout_line = help
        .lines()
        .find(|line| line.contains("--timeout-ms <TIMEOUT_MS>"))
        .expect("timeout flag line");

    assert!(timeout_line.contains("Override the runtime timeout for this command"));
}

#[test]
fn window_resize_help_shows_focus_and_verify_flags() {
    let help = command_help(["operator", "window", "resize", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window resize [OPTIONS] --width <W> --height <H>",
        "Resize a window",
        &[
            "operator window resize --window-id 42 --width 1280 --height 800",
            "operator window resize --app TextEdit --width 900 --height 600 --verify geometry",
        ],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("Target (pick one, required)"));
    assert!(help.contains("Verification"));
    assert!(help.contains("--focus auto|never"));
    assert!(help.contains("--verify geometry"));
}

#[test]
fn app_list_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "list", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app list [OPTIONS]",
        "List operable applications",
        &[
            "operator app list",
            "operator app list --running",
            "operator app list --all",
            "operator app list --name Cod",
            "operator app list --running --bundle com.apple.TextEdit",
            "operator app list --all --bundle com.apple.TextEdit",
            "operator --json app list --all",
        ],
    );
    assert!(help.contains("Mode (pick one)"));
    assert!(help.contains("Filters (optional)"));
    assert!(help.contains("--running"));
    assert!(help.contains("--all"));
    assert!(help.contains("--name <TEXT>"));
    assert!(help.contains("--bundle <BUNDLE_ID>"));
    assert!(help.contains("defaults to `--running`"));
    assert!(help.contains("switch the default view to `--all`"));
    assert!(help.contains("operable running apps"));
    assert!(help.contains("case-insensitive contains matching"));
    assert!(help.contains("bundle-id fragments"));
}

#[test]
fn capture_window_help_snapshot_is_stable() {
    let help = command_help(["operator", "capture", "window", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator capture window [OPTIONS] --window-id <ID>",
        "Take a screenshot of a specific window",
        &[
            "operator capture window --window-id 42",
            "operator --json capture window --window-id 42",
        ],
    );
    assert!(help.contains("Take a screenshot of a specific window"));
    assert!(help.contains("Options"));
    assert!(help.contains("--window-id <ID>"));
    assert!(help.contains("Select the named runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Override the runtime timeout for this command"));
}

#[test]
fn capture_frontmost_help_snapshot_is_stable() {
    let help = command_help(["operator", "capture", "frontmost", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator capture frontmost [OPTIONS]",
        "Take a screenshot of the frontmost app surface",
        &[
            "operator capture frontmost",
            "operator --json capture frontmost",
        ],
    );
    assert!(!help.contains("\nOptions\n"));
}

#[test]
fn capture_region_help_snapshot_is_stable() {
    let help = command_help(["operator", "capture", "region", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator capture region [OPTIONS] --x <X> --y <Y> --width <W> --height <H>",
        "Take a screenshot of a screen region defined by coordinates",
        &[
            "operator capture region --x 0 --y 0 --width 800 --height 600",
            "operator capture region --x 100 --y 200 --width 400 --height 300",
        ],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("--x <X>"));
    assert!(help.contains("--height <H>"));
}

#[test]
fn capture_fullscreen_help_snapshot_is_stable() {
    let help = command_help(["operator", "capture", "fullscreen", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator capture fullscreen [OPTIONS]",
        "Take a screenshot of the full display",
        &[
            "operator capture fullscreen",
            "operator capture fullscreen --display-id 2",
        ],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("--display-id <ID>"));
    assert!(help.contains("Display to capture (optional, defaults to the active display)"));
}

#[test]
fn elements_frontmost_help_snapshot_is_stable() {
    let help = command_help(["operator", "elements", "frontmost", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator elements frontmost [OPTIONS]",
        "Query the accessibility element tree for the frontmost app surface",
        &[
            "operator elements frontmost",
            "operator --json elements frontmost",
        ],
    );
    assert!(!help.contains("\nOptions\n"));
}

#[test]
fn elements_window_help_snapshot_is_stable() {
    let help = command_help(["operator", "elements", "window", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator elements window [OPTIONS] --window-id <ID>",
        "Query the accessibility element tree for a specific window",
        &[
            "operator elements window --window-id 42",
            "operator --json elements window --window-id 42",
        ],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("--window-id <ID>"));
}

#[test]
fn elements_region_help_snapshot_is_stable() {
    let help = command_help(["operator", "elements", "region", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator elements region [OPTIONS] --x <X> --y <Y> --width <W> --height <H>",
        "Query accessibility elements whose bounds intersect a screen region",
        &["operator elements region --x 0 --y 0 --width 800 --height 600"],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("--x <X>"));
    assert!(help.contains("--height <H>"));
    assert!(help.contains("macOS note: region queries enumerate visible accessible windows"));
}

#[test]
fn elements_fullscreen_help_snapshot_is_stable() {
    let help = command_help(["operator", "elements", "fullscreen", "--help"]);
    assert_surface_leaf_help_shape(
        &help,
        "Usage operator elements fullscreen [OPTIONS]",
        "Query accessibility elements across visible windows on the desktop",
        &["operator elements fullscreen"],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("--display-id <ID>"));
    assert!(help.contains("Display hint for the query (currently best-effort on macOS)"));
    assert!(help.contains("does not yet narrow the AX query"));
}

#[test]
fn snapshot_help_snapshot_is_stable() {
    let help = command_help(["operator", "snapshot", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator snapshot [OPTIONS] <SNAPSHOT-ID>",
        "Read a stored snapshot by ID",
        &[
            "operator snapshot s_abc123",
            "operator --json snapshot s_abc123",
        ],
    );
}

#[test]
fn artifact_help_snapshot_is_stable() {
    let help = command_help(["operator", "artifact", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator artifact [OPTIONS] <ARTIFACT-ID>",
        "Read a stored capture artifact by ID",
        &[
            "operator artifact capture-1.png",
            "operator --json artifact capture-1.png",
        ],
    );
}

#[test]
fn click_help_snapshot_is_stable() {
    let help = command_help(["operator", "click", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator click [OPTIONS]",
        "Click a locator, coordinates, or target",
        &[
            "operator click --text Save",
            "operator click --text \"Open File\" --app Finder --verify focus",
        ],
    );
    assert!(help.contains("Locator (pick one group)"));
    assert!(help.contains("Target (optional, defaults to frontmost)"));
    assert!(help.contains("Verification"));
    assert!(help.contains("--mode left|right|middle|double"));
}

#[test]
fn press_help_snapshot_is_stable() {
    let help = command_help(["operator", "press", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator press [OPTIONS] <KEY>",
        "Press a single key, optionally multiple times",
        &[
            "operator press return",
            "operator press escape --app Notes",
            "operator press tab --count 3",
        ],
    );
    assert!(help.contains("Arguments"));
    assert!(help.contains("Verification"));
}

#[test]
fn hotkey_help_snapshot_is_stable() {
    let help = command_help(["operator", "hotkey", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator hotkey [OPTIONS] <KEY>...",
        "Press a key chord",
        &[
            "operator hotkey command s",
            "operator hotkey command shift z --app TextEdit",
            "operator hotkey control c",
        ],
    );
    assert!(help.contains("Arguments"));
    assert!(help.contains("Verification"));
}

#[test]
fn scroll_help_snapshot_is_stable() {
    let help = command_help(["operator", "scroll", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator scroll [OPTIONS] --delta-x <DX> --delta-y <DY>",
        "Scroll by delta at a locator or target",
        &[
            "operator scroll --delta-x 0 --delta-y 300",
            "operator scroll --delta-x 0 --delta-y -200 --app Safari",
        ],
    );
    assert!(help.contains("Locator (pick one group)"));
    assert!(help.contains("Target (optional, defaults to frontmost)"));
    assert!(!help.contains("\nVerification\n"));
}

#[test]
fn drag_help_snapshot_is_stable() {
    let help = command_help(["operator", "drag", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator drag [OPTIONS]",
        "Drag from one locator to another",
        &[
            "operator drag --from-text \"file.txt\" --to-text \"Documents\"",
            "operator drag --from-x 100 --from-y 200 --to-x 400 --to-y 500",
        ],
    );
    assert!(help.contains("From Locator (pick one group, required)"));
    assert!(help.contains("To Locator (pick one group, required)"));
    assert!(!help.contains("\nVerification\n"));
}

#[test]
fn swipe_help_snapshot_is_stable() {
    let help = command_help(["operator", "swipe", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator swipe [OPTIONS]",
        "Swipe from one locator to another",
        &[
            "operator swipe --from-x 200 --from-y 500 --to-x 200 --to-y 100",
            "operator swipe --from-x 100 --from-y 300 --to-x 600 --to-y 300 --duration-ms 300",
        ],
    );
    assert!(help.contains("From Locator (pick one group, required)"));
    assert!(help.contains("To Locator (pick one group, required)"));
}

#[test]
fn move_help_snapshot_is_stable() {
    let help = command_help(["operator", "move", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator move [OPTIONS]",
        "Move the pointer to a locator or coordinates without clicking",
        &[
            "operator move --text \"Submit\"",
            "operator move --x 400 --y 300",
            "operator move --role button --index 1 --app Safari",
        ],
    );
    assert!(help.contains("Locator (pick one group, required)"));
    assert!(!help.contains("\nVerification\n"));
}

#[test]
fn app_launch_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "launch", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app launch [OPTIONS] <APP>",
        "Launch an application by name or bundle identifier",
        &[
            "operator app launch Notes",
            "operator app launch com.apple.TextEdit",
        ],
    );
    assert!(help.contains("Arguments"));
}

#[test]
fn app_quit_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "quit", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app quit [OPTIONS]",
        "Quit an application",
        &[
            "operator app quit --app Notes",
            "operator app quit --pid 1234",
        ],
    );
    assert!(help.contains("Target (pick one, required)"));
    assert!(help.contains("Verification"));
}

#[test]
fn app_hide_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "hide", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app hide [OPTIONS]",
        "Hide an application (remove from screen without quitting)",
        &["operator app hide --app Notes"],
    );
    assert!(help.contains("Target (pick one, required)"));
    assert!(!help.contains("\nVerification\n"));
}

#[test]
fn app_unhide_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "unhide", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app unhide [OPTIONS]",
        "Unhide a hidden application",
        &["operator app unhide --app Notes"],
    );
    assert!(help.contains("Target (pick one, required)"));
}

#[test]
fn app_relaunch_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "relaunch", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app relaunch [OPTIONS]",
        "Quit and relaunch an application",
        &["operator app relaunch --app Notes"],
    );
    assert!(help.contains("Target (pick one, required)"));
}

#[test]
fn app_switch_help_snapshot_is_stable() {
    let help = command_help(["operator", "app", "switch", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator app switch [OPTIONS]",
        "Bring an application to the foreground. Switches to the app's frontmost window.",
        &[
            "operator app switch --app TextEdit",
            "operator app switch --app Safari --verify focus",
        ],
    );
    assert!(
        help.contains("Use 'operator window focus' to target a specific window within the app.")
    );
    assert!(help.contains("Target (pick one, required)"));
    assert!(help.contains("--verify focus|window-state|geometry"));
}

#[test]
fn window_focus_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "focus", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window focus [OPTIONS] --window-id <ID>",
        "Bring a specific window to the foreground",
        &[
            "operator window focus --window-id 42",
            "operator window focus --window-id 42 --verify focus",
        ],
    );
    assert!(help.contains("Verification"));
    assert!(help.contains("--verify focus|window-state|geometry"));
}

#[test]
fn window_resize_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "resize", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window resize [OPTIONS] --width <W> --height <H>",
        "Resize a window",
        &[
            "operator window resize --window-id 42 --width 1280 --height 800",
            "operator window resize --app TextEdit --width 900 --height 600 --verify geometry",
        ],
    );
    assert!(help.contains("--focus auto|never"));
    assert!(help.contains("--verify geometry"));
}

#[test]
fn window_close_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "close", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window close [OPTIONS]",
        "Close a window",
        &[
            "operator window close --window-id 42",
            "operator window close --app TextEdit",
        ],
    );
    assert!(help.contains("Target (pick one, required)"));
}

#[test]
fn window_minimize_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "minimize", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window minimize [OPTIONS]",
        "Minimize a window to the Dock",
        &[
            "operator window minimize --window-id 42",
            "operator window minimize --app Notes --verify window-state",
        ],
    );
    assert!(help.contains("Verification"));
    assert!(help.contains("--verify window-state"));
}

#[test]
fn window_maximize_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "maximize", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window maximize [OPTIONS]",
        "Maximize a window to fill the display",
        &[
            "operator window maximize --window-id 42",
            "operator window maximize --app TextEdit",
        ],
    );
    assert!(help.contains("Target (pick one, required)"));
}

#[test]
fn window_move_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "move", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window move [OPTIONS] --x <X> --y <Y>",
        "Move a window to new screen coordinates",
        &[
            "operator window move --window-id 42 --x 100 --y 50",
            "operator window move --app TextEdit --x 0 --y 0 --verify geometry",
        ],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("Verification"));
}

#[test]
fn window_set_bounds_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "set-bounds", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window set-bounds [OPTIONS] --x <X> --y <Y> --width <W> --height <H>",
        "Set the full position and size of a window in one operation",
        &[
            "operator window set-bounds --window-id 42 --x 0 --y 0 --width 1280 --height 800",
            "operator window set-bounds --app Notes --x 100 --y 100 --width 800 --height 600 --verify geometry",
        ],
    );
    assert!(help.contains("Options"));
    assert!(help.contains("Verification"));
}

#[test]
fn window_list_help_snapshot_is_stable() {
    let help = command_help(["operator", "window", "list", "--help"]);
    assert_leaf_help_shape(
        &help,
        "Usage operator window list [OPTIONS] --app <NAME>",
        "List application windows",
        &[
            "operator window list --app TextEdit",
            "operator --json window list --app TextEdit",
        ],
    );
    assert!(help.contains("Target (required)"));
    assert!(help.contains("--app <NAME>"));
    assert!(help.contains("requires `--app <NAME>`"));
}

#[test]
fn mcp_serve_help_snapshot_is_stable() {
    let help = command_help(["operator", "mcp", "serve", "--help"]);
    assert!(help.contains("Start the MCP stdio server. Reads JSON-RPC messages from stdin"));
    assert!(help.contains("Select the named runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Examples\n  operator mcp serve"));
}

#[test]
fn snapshot_command_maps_positional_snapshot_id_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "snapshot",
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
    let cases: [(&[&str], &str, &str); 38] = [
        (
            &["operator", "snapshot-get", "s_123"],
            "snapshot-get",
            "operator snapshot <snapshot-id>",
        ),
        (
            &["operator", "artifact-get", "capture-1.png"],
            "artifact-get",
            "operator artifact <artifact-id>",
        ),
        (
            &["operator", "snapshot", "get", "s_123"],
            "snapshot get",
            "operator snapshot <snapshot-id>",
        ),
        (
            &["operator", "artifact", "get", "capture-1.png"],
            "artifact get",
            "operator artifact <artifact-id>",
        ),
        (
            &["operator", "observe", "frontmost"],
            "observe frontmost",
            "operator capture frontmost",
        ),
        (
            &[
                "operator",
                "observe",
                "window",
                "--window-id",
                "42",
                "--capture",
                "elements",
            ],
            "observe window",
            "operator elements window",
        ),
        (
            &[
                "operator",
                "observe",
                "region",
                "--x",
                "0",
                "--y",
                "0",
                "--width",
                "10",
                "--height",
                "10",
                "--capture",
                "all",
            ],
            "observe region",
            "operator capture region or operator elements region",
        ),
        (
            &["operator", "observe"],
            "observe",
            "operator capture <surface> or operator elements <surface>",
        ),
        (&["operator", "get-focus"], "get-focus", "operator show"),
        (&["operator", "focus"], "focus", "operator show"),
        (&["operator", "list"], "list", "operator app list or operator window list"),
        (&["operator", "list", "apps"], "list apps", "operator app list"),
        (&["operator", "list", "windows"], "list windows", "operator window list"),
        (&["operator", "list-apps"], "list-apps", "operator app list"),
        (&["operator", "list-windows"], "list-windows", "operator window list"),
        (
            &["operator", "permissions-status"],
            "permissions-status",
            "operator permissions",
        ),
        (
            &["operator", "input"],
            "input",
            "operator click, operator type, operator press, operator hotkey, operator scroll, operator drag, operator swipe, or operator move",
        ),
        (&["operator", "input", "click"], "input click", "operator click"),
        (&["operator", "input", "move"], "input move", "operator move"),
        (&["operator", "input", "type"], "input type", "operator type"),
        (&["operator", "input", "press"], "input press", "operator press"),
        (
            &["operator", "input", "hotkey"],
            "input hotkey",
            "operator hotkey",
        ),
        (
            &["operator", "input", "scroll"],
            "input scroll",
            "operator scroll",
        ),
        (&["operator", "input", "drag"], "input drag", "operator drag"),
        (
            &["operator", "input", "swipe"],
            "input swipe",
            "operator swipe",
        ),
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
        &[
            "operator",
            "--json",
            "--target",
            "local:macos",
            "input",
            "click",
        ],
        "input click",
        "operator click",
    );
}

#[tokio::test]
async fn click_command_maps_locator_target_focus_and_verification() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
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
fn click_command_rejects_conflicting_locator_variants() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "click", "--text", "Save", "--x", "24", "--y", "48",
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
async fn app_switch_command_maps_verification_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator", "app", "switch", "--app", "Safari", "--verify", "focus",
    ])
    .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "switch-app");
    assert_eq!(
        invocation.input,
        json!({
            "target_selector": {
                "App": "Safari"
            },
            "verifications": ["Focus"]
        })
    );
}

#[test]
fn app_switch_command_rejects_window_index_target_selector() {
    let error =
        cli_main::args::Cli::try_parse_from(["operator", "app", "switch", "--window-index", "1"])
            .unwrap_err();

    assert!(error
        .to_string()
        .contains("unexpected argument '--window-index'"));
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

#[test]
fn app_hide_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator", "app", "hide", "--app", "Notes", "--verify", "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[test]
fn app_unhide_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator", "app", "unhide", "--app", "Notes", "--verify", "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[test]
fn app_relaunch_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator", "app", "relaunch", "--app", "Notes", "--verify", "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
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
async fn app_hide_command_maps_window_title_target_selector_to_tool_input() {
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "app", "hide", "--window-title", "Draft"])
            .unwrap();

    let invocation = cli.into_invocation().unwrap();
    assert_eq!(invocation.tool, "hide-app");
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
async fn type_command_maps_app_target_selector_with_default_focus_policy() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
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
async fn scroll_command_maps_locator_and_deltas_to_tool_input() {
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

#[test]
fn scroll_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
        "scroll",
        "--delta-x",
        "0",
        "--delta-y",
        "-120",
        "--verify",
        "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[test]
fn scroll_command_rejects_incomplete_snapshot_locator() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
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
async fn move_command_maps_coordinate_locator() {
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

#[test]
fn move_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator", "move", "--x", "640", "--y", "480", "--verify", "focus",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
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

#[test]
fn drag_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator", "drag", "--from-x", "1", "--from-y", "2", "--to-x", "3", "--to-y", "4",
        "--verify", "geometry",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
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

#[test]
fn swipe_command_rejects_verification_flags() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator", "swipe", "--from-x", "1", "--from-y", "2", "--to-x", "3", "--to-y", "4",
        "--verify", "geometry",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("unexpected argument '--verify'"));
}

#[tokio::test]
async fn hotkey_command_maps_positional_keys_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
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
async fn press_command_maps_positional_key_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
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
fn type_command_rejects_legacy_trailing_key_flag() {
    let error = cli_main::args::Cli::try_parse_from([
        "operator",
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
async fn type_command_maps_positional_text_after_keys_and_locator_to_tool_input() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
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
async fn cli_run_executes_agent_command_and_renders_text_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "agent",
        "--target",
        "local:macos",
        "--timeout-ms",
        "250",
        "--model",
        "doubao-seed",
        "--max-steps",
        "8",
        "Summarize the frontmost window",
    ])
    .unwrap();

    let tool_invoker = RecordingInvoker {
        calls: Arc::new(Mutex::new(Vec::new())),
        response: json!({}),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let executor = RecordingAgentExecutor {
        calls: Arc::clone(&calls),
        result: Ok(AgentRunResult {
            session_id: SessionId("sess-1".into()),
            target: TargetId("local:macos".into()),
            model: "doubao-seed".into(),
            summary: "Observed the frontmost window.".into(),
        }),
    };

    let rendered = cli_main::run_with_handlers(cli, &tool_invoker, &executor)
        .await
        .unwrap();
    assert_eq!(
        rendered,
        "session_id: sess-1\ntarget: local:macos\nmodel: doubao-seed\nsummary: Observed the frontmost window."
    );

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].task, "Summarize the frontmost window");
    assert_eq!(recorded[0].model.as_deref(), Some("doubao-seed"));
    assert_eq!(recorded[0].max_steps, Some(NonZeroU32::new(8).unwrap()));
    assert_eq!(recorded[0].target.as_deref(), Some("local:macos"));
    assert_eq!(recorded[0].timeout_ms, Some(250));
    assert!(!recorded[0].json_output);
}

#[tokio::test]
async fn cli_run_executes_agent_command_and_renders_json_output() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "agent",
        "--json",
        "Open Notes and type hello",
    ])
    .unwrap();

    let tool_invoker = RecordingInvoker {
        calls: Arc::new(Mutex::new(Vec::new())),
        response: json!({}),
    };
    let executor = RecordingAgentExecutor {
        calls: Arc::new(Mutex::new(Vec::new())),
        result: Ok(AgentRunResult {
            session_id: SessionId("sess-42".into()),
            target: TargetId("local:macos".into()),
            model: "gpt-5.4".into(),
            summary: "Opened Notes and typed hello.".into(),
        }),
    };

    let rendered = cli_main::run_with_handlers(cli, &tool_invoker, &executor)
        .await
        .unwrap();
    let output = serde_json::from_str::<Value>(&rendered).unwrap();
    assert_eq!(
        output,
        json!({
            "session_id": "sess-42",
            "target": "local:macos",
            "model": "gpt-5.4",
            "summary": "Opened Notes and typed hello."
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
        cli_main::args::Cli::try_parse_from(["operator", "press", "down", "--count", "3"]).unwrap();

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
    let cli =
        cli_main::args::Cli::try_parse_from(["operator", "artifact", "capture-1.png"]).unwrap();

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
async fn cli_run_renders_show_summary_for_non_json_output() {
    let cli = cli_main::args::Cli::try_parse_from(["operator", "show"]).unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = RecordingInvoker {
        calls: Arc::clone(&calls),
        response: json!({
            "focus": {
                "role": "AXTextField",
                "label": "Search",
                "bundle_id": "com.apple.Safari",
                "app_name": "Safari",
                "bounds": {
                    "x": 40.0,
                    "y": 60.0,
                    "width": 280.0,
                    "height": 32.0
                }
            }
        }),
    };

    let rendered = cli_main::run_with_invoker(cli, &invoker).await.unwrap();
    assert_eq!(rendered, "Safari\tAXTextField\tSearch");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded[0].0, "get-focus");
    assert_eq!(recorded[0].1, json!({}));
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

#[tokio::test]
async fn cli_run_distinguishes_known_target_with_missing_driver() {
    let cli = cli_main::args::Cli::try_parse_from([
        "operator",
        "capabilities",
        "--target",
        "windows-lab",
    ])
    .unwrap();

    let runtime = RuntimeBuilder::new(RuntimeConfig {
        default_target: TargetId("windows-lab".into()),
        targets: BTreeMap::from([(
            "windows-lab".into(),
            NamedTargetConfig {
                platform: "windows".into(),
                driver: "windows.remote".into(),
                driver_config: DriverConfig::new(),
            },
        )]),
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .build()
    .await
    .unwrap();
    let invoker = RuntimeBackedInvoker {
        tools: runtime.tools().clone(),
    };

    let error = cli_main::run_with_invoker(cli, &invoker)
        .await
        .expect_err("missing driver should surface as an operator error");

    match error {
        cli_main::CliError::Operator(OperatorError::DriverUnavailable { target, driver }) => {
            assert_eq!(target, "windows-lab");
            assert_eq!(driver, "windows.remote");
        }
        other => panic!("expected driver unavailable, got {other:?}"),
    }
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

struct RuntimeBackedInvoker {
    tools: ToolRegistry,
}

impl cli_main::ToolInvoker for RuntimeBackedInvoker {
    fn invoke<'a>(
        &'a self,
        tool: &'a str,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, operator_core::OperatorError>> + Send + 'a>>
    {
        Box::pin(async move { self.tools.invoke(tool, input).await })
    }
}

struct RecordingAgentExecutor {
    calls: Arc<Mutex<Vec<cli_main::args::AgentCommand>>>,
    result: Result<AgentRunResult, String>,
}

impl cli_main::AgentExecutor for RecordingAgentExecutor {
    fn run<'a>(
        &'a self,
        command: &'a cli_main::args::AgentCommand,
    ) -> Pin<Box<dyn Future<Output = Result<AgentRunResult, String>> + Send + 'a>> {
        self.calls.lock().unwrap().push(command.clone());
        let result = self.result.clone();
        Box::pin(async move { result })
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

fn assert_leaf_help_shape(help: &str, usage: &str, about: &str, examples: &[&str]) {
    assert!(help.contains(usage));
    assert!(help.contains(about));
    assert!(help.contains("Global Runtime Flags"));
    assert!(help.contains("Select the named runtime target"));
    assert!(help.contains("Emit machine-readable JSON output"));
    assert!(help.contains("Override the runtime timeout for this command"));
    assert!(help.contains("Print help"));
    assert!(help.contains("Examples"));
    for example in examples {
        assert!(help.contains(example), "missing example: {example}");
    }
}

fn assert_surface_leaf_help_shape(help: &str, usage: &str, about: &str, examples: &[&str]) {
    assert_leaf_help_shape(help, usage, about, examples);
}

fn assert_legacy_command_migration(args: &[&str], legacy: &str, replacement: &str) {
    let error = cli_main::args::Cli::try_parse_from(args.iter().copied()).unwrap_err();
    let message = error.to_string();
    let legacy_path = format!("operator {legacy}");

    assert!(
        message.contains(&legacy_path),
        "missing legacy command in `{message}`"
    );
    assert!(
        message.contains(replacement),
        "missing replacement command in `{message}`"
    );
}
