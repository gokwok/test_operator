use std::{io::Cursor, sync::Arc};

use operator_mcp::{run_stdio_session, McpServer};
use operator_runtime::{RuntimeBuilder, RuntimeConfig};
use operator_testkit::InMemorySnapshotStore;
use serde_json::{json, Value};

#[test]
fn tools_list_requires_initialized_notification() {
    let mut server = discovery_server();

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
}

#[test]
fn initialize_rejects_unsupported_protocol_versions() {
    let mut server = discovery_server();

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
    let mut server = discovery_server();

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
    let mut server = discovery_server();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n"
    );

    let mut output = Vec::new();
    run_stdio_session(&mut server, Cursor::new(input.as_bytes()), &mut output).unwrap();

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
    assert!(tools.iter().any(|tool| tool["name"] == json!("observe")));
    assert!(tools.iter().any(|tool| tool["name"] == json!("click")));

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
}

#[test]
fn stdio_transport_emits_parse_errors_without_terminating_session() {
    let mut server = discovery_server();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"broken\":\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n"
    );

    let mut output = Vec::new();
    run_stdio_session(&mut server, Cursor::new(input.as_bytes()), &mut output).unwrap();

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

fn discovery_server() -> McpServer {
    let runtime = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            RuntimeBuilder::new(RuntimeConfig::default())
                .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
                .build()
                .await
        })
        .unwrap();

    McpServer::new(runtime.tools().specs())
}
