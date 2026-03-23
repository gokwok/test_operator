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
            "button": "Left",
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
