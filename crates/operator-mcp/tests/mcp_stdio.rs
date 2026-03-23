use std::{
    io::Cursor,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use operator_core::{
    ActionOutcome, ActionRequest, Capability, CapabilitySet, ExecContext, HealthStatus,
    ObserveRequest, ObserveResult, OperatorError, PermissionStatus, PermissionsReport,
    PlatformDriver, QueryRequest, QueryResult,
};
use operator_mcp::{run_stdio_session, McpServer};
use operator_runtime::SnapshotStore;
use operator_runtime::{FileArtifactStore, RuntimeBuilder, RuntimeConfig};
use operator_testkit::{test_snapshot, InMemorySnapshotStore};
use serde_json::{json, Value};
use tokio::sync::Notify;

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
                    "button": "Left"
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

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new([Capability::WindowManagement])
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        Ok(HealthStatus {
            healthy: true,
            message: None,
            permissions: PermissionsReport {
                screen_recording: PermissionStatus::Granted,
                accessibility: PermissionStatus::Granted,
            },
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
