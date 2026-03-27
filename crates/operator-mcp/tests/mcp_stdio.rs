use std::{
    io::Cursor,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionCoordinates, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionSideEffect,
    ActionTargetSelector, ActionVerification, Capability, CapabilitySet, ExecContext, HealthStatus,
    Locator, ObserveRequest, ObserveResult, OperatorError, PermissionCheck, PermissionStatus,
    PermissionsReport, PlatformDriver, Point, QueryRequest, QueryResult, Rect, TypeTrailingKey,
    WindowInfo,
};
use operator_mcp::{run_stdio_session, McpServer};
use operator_runtime::SnapshotStore;
use operator_runtime::{FileArtifactStore, RuntimeBuilder, RuntimeConfig};
use operator_testkit::{test_snapshot, InMemorySnapshotStore, MockPlatformDriver};
use serde_json::{json, Value};
use tokio::sync::Notify;

fn default_action_request() -> ActionRequest {
    ActionRequest {
        action: Action::Move,
        locator: None,
        target_selector: None,
        focus_policy: ActionFocusPolicy::Auto,
        verifications: Vec::new(),
    }
}

fn successful_action_outcome(detail: &str, duration_ms: u64) -> ActionOutcome {
    ActionOutcome {
        success: true,
        duration_ms,
        detail: Some(detail.into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }
}

fn schema_ref<'a>(schema: &'a Value, reference: &str) -> &'a Value {
    let key = reference.rsplit('/').next().unwrap();
    schema
        .get("$defs")
        .and_then(|defs| defs.get(key))
        .or_else(|| schema.get("definitions").and_then(|defs| defs.get(key)))
        .unwrap_or_else(|| panic!("missing schema reference: {reference}"))
}

fn verification_enum_values(schema: &Value) -> Vec<String> {
    let verifications = &schema["properties"]["verifications"];
    if verifications.is_null() {
        return Vec::new();
    }

    let items = &verifications["items"];
    let enum_schema = if let Some(reference) = items["$ref"].as_str() {
        schema_ref(schema, reference)
    } else {
        items
    };

    enum_schema["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn tools_list_requires_initialized_notification() {
    let server = discovery_server();

    let init_response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "0.1.0"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        init_response["result"]["capabilities"]["tools"]["listChanged"],
        json!(false)
    );
    assert_eq!(
        init_response["result"]["serverInfo"]["name"],
        json!("operator-mcp")
    );

    let list_response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }))
        .unwrap()
        .unwrap();

    assert_eq!(list_response["error"]["code"], json!(-32002));
    assert_eq!(
        list_response["error"]["message"],
        json!("server not initialized")
    );

    let call_response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "snapshot-get",
                "arguments": {
                    "snapshot_id": "snap-1"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(call_response["error"]["code"], json!(-32002));
    assert_eq!(
        call_response["error"]["message"],
        json!("server not initialized")
    );
}

#[test]
fn initialize_rejects_unsupported_protocol_versions() {
    let server = discovery_server();

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "initialize",
            "params": {
                "protocolVersion": "1.0.0",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "0.1.0"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(response["error"]["code"], json!(-32602));
    assert_eq!(
        response["error"]["data"]["supported"],
        json!(["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"])
    );
    assert_eq!(response["error"]["data"]["requested"], json!("1.0.0"));
}

#[test]
fn initialize_requires_protocol_version_param() {
    let server = discovery_server();

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "initialize",
            "params": {
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "0.1.0"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(response["error"]["code"], json!(-32602));
    assert_eq!(
        response["error"]["message"],
        json!("initialize params are invalid")
    );
}

#[test]
fn stdio_transport_round_trips_initialize_and_tools_list() {
    let server = discovery_server();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n"
    );

    let mut output = Vec::new();
    run_stdio_session(&server, Cursor::new(input.as_bytes()), &mut output).unwrap();

    let responses = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[0]["result"]["protocolVersion"],
        json!("2025-11-25")
    );

    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("artifact-get")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("observe")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("click")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("close-window")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("maximize-window")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("minimize-window")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("move")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("move-window")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("resize-window")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("set-window-bounds")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("switch-app")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("quit-app")));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("relaunch-app")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("hide-app")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("unhide-app")));

    let artifact_get = tools
        .iter()
        .find(|tool| tool["name"] == json!("artifact-get"))
        .unwrap();
    assert_eq!(artifact_get["annotations"]["readOnlyHint"], json!(true));

    let observe = tools
        .iter()
        .find(|tool| tool["name"] == json!("observe"))
        .unwrap();
    assert_eq!(observe["annotations"]["readOnlyHint"], json!(true));

    let click = tools
        .iter()
        .find(|tool| tool["name"] == json!("click"))
        .unwrap();
    assert_eq!(click["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(click["annotations"]["destructiveHint"], json!(true));
    assert!(click["inputSchema"]["properties"]["mode"].is_object());
    assert!(click["inputSchema"]["properties"]["button"].is_null());
    assert!(click["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(click["inputSchema"]["properties"]["focus_policy"].is_object());
    assert!(click["inputSchema"]["properties"]["verifications"].is_object());

    let scroll = tools
        .iter()
        .find(|tool| tool["name"] == json!("scroll"))
        .unwrap();
    assert!(scroll["inputSchema"]["properties"]["locator"].is_object());

    let move_tool = tools
        .iter()
        .find(|tool| tool["name"] == json!("move"))
        .unwrap();
    assert_eq!(move_tool["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(move_tool["annotations"]["destructiveHint"], json!(true));
    assert!(move_tool["inputSchema"]["properties"]["locator"].is_object());
    assert!(move_tool["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(move_tool["inputSchema"]["properties"]["focus_policy"].is_object());
    assert!(move_tool["inputSchema"]["properties"]["verifications"].is_object());

    let drag = tools
        .iter()
        .find(|tool| tool["name"] == json!("drag"))
        .unwrap();
    assert!(drag["inputSchema"]["properties"]["duration_ms"].is_object());
    assert!(drag["inputSchema"]["properties"]["steps"].is_object());
    assert!(drag["inputSchema"]["properties"]["modifiers"].is_object());

    let launch_app = tools
        .iter()
        .find(|tool| tool["name"] == json!("launch-app"))
        .unwrap();
    assert!(launch_app["inputSchema"]["properties"]["verifications"].is_null());

    let close_window = tools
        .iter()
        .find(|tool| tool["name"] == json!("close-window"))
        .unwrap();
    assert_eq!(close_window["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(close_window["annotations"]["destructiveHint"], json!(true));
    assert!(close_window["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(close_window["inputSchema"]["properties"]["focus_policy"].is_object());
    assert!(close_window["inputSchema"]["properties"]["verifications"].is_null());

    let minimize_window = tools
        .iter()
        .find(|tool| tool["name"] == json!("minimize-window"))
        .unwrap();
    assert_eq!(minimize_window["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(
        minimize_window["annotations"]["destructiveHint"],
        json!(true)
    );
    assert!(minimize_window["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(minimize_window["inputSchema"]["properties"]["focus_policy"].is_object());
    assert_eq!(
        verification_enum_values(&minimize_window["inputSchema"]),
        vec!["WindowState"]
    );

    let maximize_window = tools
        .iter()
        .find(|tool| tool["name"] == json!("maximize-window"))
        .unwrap();
    assert_eq!(maximize_window["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(
        maximize_window["annotations"]["destructiveHint"],
        json!(true)
    );
    assert!(maximize_window["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(maximize_window["inputSchema"]["properties"]["focus_policy"].is_object());
    assert!(maximize_window["inputSchema"]["properties"]["verifications"].is_null());

    for tool_name in ["move-window", "resize-window", "set-window-bounds"] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == json!(tool_name))
            .unwrap();
        assert_eq!(tool["annotations"]["readOnlyHint"], json!(false));
        assert_eq!(tool["annotations"]["destructiveHint"], json!(true));
        assert!(tool["inputSchema"]["properties"]["target_selector"].is_object());
        assert!(tool["inputSchema"]["properties"]["focus_policy"].is_object());
    }

    let move_window = tools
        .iter()
        .find(|tool| tool["name"] == json!("move-window"))
        .unwrap();
    assert!(move_window["inputSchema"]["properties"]["x"].is_object());
    assert!(move_window["inputSchema"]["properties"]["y"].is_object());

    let resize_window = tools
        .iter()
        .find(|tool| tool["name"] == json!("resize-window"))
        .unwrap();
    assert!(resize_window["inputSchema"]["properties"]["width"].is_object());
    assert!(resize_window["inputSchema"]["properties"]["height"].is_object());

    let set_window_bounds = tools
        .iter()
        .find(|tool| tool["name"] == json!("set-window-bounds"))
        .unwrap();
    assert!(set_window_bounds["inputSchema"]["properties"]["x"].is_object());
    assert!(set_window_bounds["inputSchema"]["properties"]["y"].is_object());
    assert!(set_window_bounds["inputSchema"]["properties"]["width"].is_object());
    assert!(set_window_bounds["inputSchema"]["properties"]["height"].is_object());

    let press = tools
        .iter()
        .find(|tool| tool["name"] == json!("press"))
        .unwrap();
    assert_eq!(press["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(press["annotations"]["destructiveHint"], json!(true));
    assert!(press["inputSchema"]["properties"]["key"].is_object());
    assert!(press["inputSchema"]["properties"]["count"].is_object());
    assert!(press["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(press["inputSchema"]["properties"]["focus_policy"].is_object());

    let type_tool = tools
        .iter()
        .find(|tool| tool["name"] == json!("type"))
        .unwrap();
    assert_eq!(type_tool["annotations"]["readOnlyHint"], json!(false));
    assert_eq!(type_tool["annotations"]["destructiveHint"], json!(true));
    assert!(type_tool["inputSchema"]["properties"]["clear_before"].is_object());
    assert!(type_tool["inputSchema"]["properties"]["delay_ms"].is_object());
    assert!(type_tool["inputSchema"]["properties"]["trailing_keys"].is_object());
    assert!(type_tool["inputSchema"]["properties"]["target_selector"].is_object());
    assert!(type_tool["inputSchema"]["properties"]["focus_policy"].is_object());

    let swipe = tools
        .iter()
        .find(|tool| tool["name"] == json!("swipe"))
        .unwrap();
    assert!(swipe["inputSchema"]["properties"]["duration_ms"].is_object());
    assert!(swipe["inputSchema"]["properties"]["steps"].is_object());
    assert!(swipe["inputSchema"]["properties"]["modifiers"].is_null());

    for tool_name in [
        "switch-app",
        "quit-app",
        "relaunch-app",
        "hide-app",
        "unhide-app",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == json!(tool_name))
            .unwrap();
        assert_eq!(tool["annotations"]["readOnlyHint"], json!(false));
        assert_eq!(tool["annotations"]["destructiveHint"], json!(true));
        assert!(tool["inputSchema"]["properties"]["target_selector"].is_object());
        assert!(tool["inputSchema"]["properties"]["focus_policy"].is_null());
    }
}

#[test]
fn stdio_transport_emits_parse_errors_without_terminating_session() {
    let server = discovery_server();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"broken\":\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n"
    );

    let mut output = Vec::new();
    run_stdio_session(&server, Cursor::new(input.as_bytes()), &mut output).unwrap();

    let responses = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1]["error"]["code"], json!(-32700));
    assert_eq!(responses[1]["id"], Value::Null);
    assert!(responses[2]["result"]["tools"].is_array());
}

#[test]
fn tools_call_executes_runtime_tools_and_returns_structured_content() {
    let server = initialized_server_with_snapshots(&["snap-1"]);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "snapshot-get",
                "arguments": {
                    "snapshot_id": "snap-1"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["snapshot"]["id"],
        json!("snap-1")
    );
    assert_eq!(response["result"]["content"][0]["type"], json!("text"));
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("\"snap-1\""));
    assert!(response["result"].get("isError").is_none());
}

#[test]
fn tools_call_executes_artifact_get_and_returns_structured_content() {
    let artifact_id = unique_artifact_id("artifact-mcp");
    let (server, artifact_path) = initialized_server_with_artifacts(&artifact_id);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 17,
            "method": "tools/call",
            "params": {
                "name": "artifact-get",
                "arguments": {
                    "artifact_id": artifact_id
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["artifact"]["id"],
        json!(artifact_id)
    );
    assert_eq!(
        response["result"]["structuredContent"]["artifact"]["path"],
        json!(artifact_path.to_string_lossy().to_string())
    );
    assert!(response["result"].get("isError").is_none());

    std::fs::remove_file(artifact_path).unwrap();
}

#[test]
fn tools_call_reports_invalid_artifact_ids_as_tool_errors() {
    let server = initialized_server_with_file_artifact_store("valid-artifact.png");

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 18,
            "method": "tools/call",
            "params": {
                "name": "artifact-get",
                "arguments": {
                    "artifact_id": "../escape.png"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(response["result"]["isError"], json!(true));
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("invalid artifact id"));
}

#[test]
fn tools_call_wraps_runtime_failures_as_tool_results() {
    let server = initialized_server_with_snapshots(&[]);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": {
                "name": "snapshot-get",
                "arguments": {
                    "snapshot_id": "missing"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(response["result"]["isError"], json!(true));
    assert_eq!(response["result"]["content"][0]["type"], json!("text"));
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("snapshot not found"));
    assert!(response["result"].get("structuredContent").is_none());
}

#[test]
fn tools_call_rejects_unknown_tools_as_protocol_errors() {
    let server = initialized_server_with_snapshots(&[]);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "tools/call",
            "params": {
                "name": "missing-tool"
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(response["error"]["code"], json!(-32602));
    assert_eq!(
        response["error"]["message"],
        json!("tool is not registered")
    );
}

#[test]
fn mcp_blocks_side_effect_tools_when_security_mode_is_disabled() {
    let runtime = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            RuntimeBuilder::new(RuntimeConfig::default())
                .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
                .build()
                .await
        })
        .unwrap();

    let server = McpServer::new(runtime.tools().clone()).with_allow_side_effects(false);
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 14,
            "method": "tools/call",
            "params": {
                "name": "click",
                "arguments": {
                    "mode": "Left"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(response["result"]["isError"], json!(true));
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("side effects are disabled by runtime policy"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_move_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::PointerInput,
            Capability::WindowManagement,
            Capability::InspectTree,
        ]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 4,
        detail: Some("moved".into()),
        coordinates: Some(ActionCoordinates {
            point: Some(Point { x: 320.0, y: 240.0 }),
            from: None,
            to: None,
        }),
        target_app: None,
        target_window: Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 120.0,
                y: 80.0,
                width: 400.0,
                height: 300.0,
            }),
            is_focused: true,
            is_minimized: false,
        }),
        side_effects: vec![ActionSideEffect::MoveCursor],
        warnings: Vec::new(),
    }));
    driver.push_query_result(Ok(QueryResult::Windows(vec![WindowInfo {
        id: 42.into(),
        title: Some("Draft".into()),
        app_name: Some("TextEdit".into()),
        bounds: Some(Rect {
            x: 120.0,
            y: 80.0,
            width: 400.0,
            height: 300.0,
        }),
        is_focused: true,
        is_minimized: false,
    }])));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 19,
            "method": "tools/call",
            "params": {
                "name": "move",
                "arguments": {
                    "target": "local:macos",
                    "target_selector": {
                        "WindowIndex": 1
                    },
                    "focus_policy": "Auto",
                    "verifications": ["Focus"],
                    "locator": {
                        "Coords": {
                            "x": 320.0,
                            "y": 240.0
                        }
                    }
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("moved")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::Move,
                locator: Some(Locator::Coords(Point { x: 320.0, y: 240.0 })),
                target_selector: Some(ActionTargetSelector::WindowIndex(1)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: vec![ActionVerification::Focus],
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
    assert_eq!(
        driver.query_calls().await,
        vec![(
            QueryRequest::ListWindows {
                app: Some("TextEdit".into()),
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_returns_richer_action_outcomes_in_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(ActionOutcome {
        success: true,
        duration_ms: 4,
        detail: Some("moved".into()),
        coordinates: Some(ActionCoordinates {
            point: Some(Point { x: 320.0, y: 240.0 }),
            from: None,
            to: None,
        }),
        target_app: None,
        target_window: Some(WindowInfo {
            id: 42.into(),
            title: Some("Draft".into()),
            app_name: Some("TextEdit".into()),
            bounds: Some(Rect {
                x: 120.0,
                y: 80.0,
                width: 400.0,
                height: 300.0,
            }),
            is_focused: true,
            is_minimized: false,
        }),
        side_effects: vec![ActionSideEffect::MoveCursor],
        warnings: vec!["locator matched fallback element".into()],
    }));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 119,
            "method": "tools/call",
            "params": {
                "name": "move",
                "arguments": {
                    "target": "local:macos",
                    "locator": {
                        "Coords": {
                            "x": 320.0,
                            "y": 240.0
                        }
                    }
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["coordinates"]["point"],
        json!({
            "x": 320.0,
            "y": 240.0
        })
    );
    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["target_window"],
        json!({
            "id": 42,
            "title": "Draft",
            "app_name": "TextEdit",
            "bounds": {
                "x": 120.0,
                "y": 80.0,
                "width": 400.0,
                "height": 300.0
            },
            "is_focused": true,
            "is_minimized": false
        })
    );
    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["side_effects"],
        json!([
            {
                "kind": "MoveCursor"
            }
        ])
    );
    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["warnings"],
        json!(["locator matched fallback element"])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_type_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::KeyboardInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("typed text", 8)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "type",
                "arguments": {
                    "target": "local:macos",
                    "text": "hello world",
                    "clear_before": true,
                    "delay_ms": 25,
                    "trailing_keys": ["Return", "Tab"],
                    "target_selector": {
                        "App": "TextEdit"
                    },
                    "focus_policy": "Auto",
                    "locator": {
                        "Text": "Search"
                    }
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("typed text")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::Type {
                    text: "hello world".into(),
                    clear_before: true,
                    delay_ms: Some(25),
                    trailing_keys: vec![TypeTrailingKey::Return, TypeTrailingKey::Tab],
                },
                locator: Some(Locator::Text("Search".into())),
                target_selector: Some(ActionTargetSelector::App("TextEdit".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_press_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::KeyboardInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("pressed down 3 times", 5)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": {
                "name": "press",
                "arguments": {
                    "target": "local:macos",
                    "key": "down",
                    "count": 3
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("pressed down 3 times")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::Press {
                    key: "down".into(),
                    count: 3.try_into().unwrap(),
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_swipe_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("swiped", 6)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "tools/call",
            "params": {
                "name": "swipe",
                "arguments": {
                    "target": "local:macos",
                    "from": {
                        "Coords": {
                            "x": 15.0,
                            "y": 25.0
                        }
                    },
                    "to": {
                        "Coords": {
                            "x": 90.0,
                            "y": 25.0
                        }
                    },
                    "duration_ms": 240,
                    "steps": 4
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("swiped")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::Swipe {
                    from: Locator::Coords(Point { x: 15.0, y: 25.0 }),
                    to: Locator::Coords(Point { x: 90.0, y: 25.0 }),
                    duration_ms: Some(240),
                    steps: Some(4.try_into().unwrap()),
                },
                locator: None,
                ..default_action_request()
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_close_window_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::WindowManagement]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("closed window 42", 10)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 23,
            "method": "tools/call",
            "params": {
                "name": "close-window",
                "arguments": {
                    "target": "local:macos",
                    "target_selector": {
                        "WindowTitle": "Draft"
                    },
                    "focus_policy": "Never"
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("closed window 42")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::CloseWindow,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Never,
                verifications: Vec::new(),
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_set_window_bounds_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::WindowManagement]),
    ));
    driver.push_action_result(Ok(successful_action_outcome(
        "set window 42 bounds to x=80 y=120 width=900 height=700",
        10,
    )));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "tools/call",
            "params": {
                "name": "set-window-bounds",
                "arguments": {
                    "target": "local:macos",
                    "target_selector": {
                        "Pid": 101
                    },
                    "focus_policy": "Auto",
                    "x": 80.0,
                    "y": 120.0,
                    "width": 900.0,
                    "height": 700.0
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("set window 42 bounds to x=80 y=120 width=900 height=700")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::SetWindowBounds {
                    bounds: Rect {
                        x: 80.0,
                        y: 120.0,
                        width: 900.0,
                        height: 700.0,
                    },
                },
                locator: None,
                target_selector: Some(ActionTargetSelector::Pid(101)),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_call_executes_switch_app_and_returns_structured_content() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_action_result(Ok(successful_action_outcome("switched app", 10)));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);

    let response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "tools/call",
            "params": {
                "name": "switch-app",
                "arguments": {
                    "target": "local:macos",
                    "target_selector": {
                        "WindowTitle": "Draft"
                    }
                }
            }
        }))
        .unwrap()
        .unwrap();

    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["detail"],
        json!("switched app")
    );
    assert!(response["result"].get("isError").is_none());
    assert_eq!(
        driver.action_calls().await,
        vec![(
            ActionRequest {
                action: Action::SwitchApp,
                locator: None,
                target_selector: Some(ActionTargetSelector::WindowTitle("Draft".into())),
                focus_policy: ActionFocusPolicy::Auto,
                verifications: Vec::new(),
            },
            ExecContext {
                target: "local:macos".into(),
                session: None,
                timeout_ms: Some(10_000),
            },
        )]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_serializes_same_target_requests() {
    let driver = Arc::new(BlockingQueryDriver::default());
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);
    let server = Arc::new(server);

    let first_server = Arc::clone(&server);
    let first = tokio::task::spawn_blocking(move || {
        first_server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 15,
                "method": "tools/call",
                "params": {
                    "name": "list-windows",
                    "arguments": {
                        "target": "local:slow",
                        "timeout_ms": 200
                    }
                }
            }))
            .unwrap()
            .unwrap()
    });

    driver.wait_until_query_starts().await;

    let second_server = Arc::clone(&server);
    let second = tokio::task::spawn_blocking(move || {
        second_server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 16,
                "method": "tools/call",
                "params": {
                    "name": "list-windows",
                    "arguments": {
                        "target": "local:slow",
                        "timeout_ms": 10
                    }
                }
            }))
            .unwrap()
            .unwrap()
    });

    let second_response = second.await.unwrap();
    assert_eq!(second_response["result"]["isError"], json!(true));
    assert!(second_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("target is busy"));

    driver.release_query();

    let first_response = first.await.unwrap();
    assert!(first_response["result"]["structuredContent"]["windows"].is_array());
}

fn discovery_server() -> McpServer {
    build_server(Arc::new(InMemorySnapshotStore::new()))
}

fn initialized_server_with_snapshots(snapshot_ids: &[&str]) -> McpServer {
    let store = Arc::new(InMemorySnapshotStore::new());
    let runtime = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            for snapshot_id in snapshot_ids {
                store.save(&test_snapshot(snapshot_id)).await.unwrap();
            }

            RuntimeBuilder::new(RuntimeConfig::default())
                .snapshot_store(store.clone())
                .build()
                .await
        })
        .unwrap();
    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);
    server
}

fn build_server(store: Arc<InMemorySnapshotStore>) -> McpServer {
    let runtime = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            RuntimeBuilder::new(RuntimeConfig::default())
                .snapshot_store(store)
                .build()
                .await
        })
        .unwrap();

    McpServer::new(runtime.tools().clone())
}

fn initialized_server_with_artifacts(artifact_id: &str) -> (McpServer, std::path::PathBuf) {
    let root = std::env::temp_dir().join(unique_artifact_id("operator-mcp-artifacts"));
    let store = Arc::new(InMemorySnapshotStore::with_artifacts_root(&root));
    std::fs::create_dir_all(&root).unwrap();
    let artifact_path = root.join(artifact_id);
    std::fs::write(&artifact_path, b"png-bytes").unwrap();

    let runtime = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            RuntimeBuilder::new(RuntimeConfig::default())
                .snapshot_store(store)
                .build()
                .await
        })
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);
    (server, artifact_path)
}

fn initialized_server_with_file_artifact_store(artifact_id: &str) -> McpServer {
    let root = std::env::temp_dir().join(unique_artifact_id("operator-mcp-artifacts-file-store"));
    let artifact_store = Arc::new(FileArtifactStore::new(&root));
    let artifacts_dir = artifact_store.artifacts_dir();
    std::fs::create_dir_all(&artifacts_dir).unwrap();
    std::fs::write(artifacts_dir.join(artifact_id), b"png-bytes").unwrap();

    let runtime = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            RuntimeBuilder::new(RuntimeConfig::default())
                .artifact_store(artifact_store)
                .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
                .build()
                .await
        })
        .unwrap();

    let server = McpServer::new(runtime.tools().clone());
    initialize_server(&server);
    server
}

fn initialize_server(server: &McpServer) {
    let init_response = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "0.1.0"
                }
            }
        }))
        .unwrap()
        .unwrap();
    assert_eq!(
        init_response["result"]["protocolVersion"],
        json!("2025-11-25")
    );

    let notification = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .unwrap();
    assert!(notification.is_none());
}

fn unique_artifact_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}.png")
}

#[derive(Default)]
struct BlockingQueryDriver {
    started: Notify,
    release: Notify,
}

impl BlockingQueryDriver {
    async fn wait_until_query_starts(&self) {
        self.started.notified().await;
    }

    fn release_query(&self) {
        self.release.notify_waiters();
    }
}

#[async_trait]
impl PlatformDriver for BlockingQueryDriver {
    fn platform_id(&self) -> &'static str {
        "slow"
    }

    fn driver_id(&self) -> &str {
        "slow.system"
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new([Capability::WindowManagement])
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        Ok(HealthStatus {
            healthy: true,
            message: None,
            permissions: PermissionsReport::new([
                PermissionCheck::new("accessibility", "Accessibility", PermissionStatus::Granted),
                PermissionCheck::new("system_events", "System Events", PermissionStatus::Granted),
                PermissionCheck::new(
                    "screen_recording",
                    "Screen Recording",
                    PermissionStatus::Granted,
                ),
            ]),
        })
    }

    async fn observe(
        &self,
        _: ObserveRequest,
        _: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        Err(OperatorError::Platform("observe unused in test".into()))
    }

    async fn query(&self, _: QueryRequest, _: &ExecContext) -> Result<QueryResult, OperatorError> {
        self.started.notify_one();
        self.release.notified().await;
        Ok(QueryResult::Windows(Vec::new()))
    }

    async fn act(&self, _: ActionRequest, _: &ExecContext) -> Result<ActionOutcome, OperatorError> {
        Err(OperatorError::Platform("act unused in test".into()))
    }
}
