use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};

use operator_core::{OperatorError, TargetId};
use operator_runtime::{RuntimeConfig, ToolRegistry, ToolSpec};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Map, Value};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::{self, Duration};

const JSONRPC_VERSION: &str = "2.0";
const PARSE_ERROR: i64 = -32700;
const SERVER_NOT_INITIALIZED: i64 = -32002;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub struct McpServer {
    tools: ToolRegistry,
    state: Mutex<ServerState>,
    allow_side_effects: bool,
    default_target: TargetId,
    default_timeout_ms: u64,
    target_serializers: Mutex<HashMap<TargetId, Arc<Semaphore>>>,
}

impl McpServer {
    pub fn new(tools: ToolRegistry) -> Self {
        let defaults = RuntimeConfig::default();
        Self {
            tools,
            state: Mutex::new(ServerState::default()),
            allow_side_effects: defaults.allow_side_effects,
            default_target: defaults.default_target,
            default_timeout_ms: defaults.default_timeout_ms,
            target_serializers: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_allow_side_effects(mut self, allow_side_effects: bool) -> Self {
        self.allow_side_effects = allow_side_effects;
        self
    }

    pub fn with_default_target(mut self, default_target: TargetId) -> Self {
        self.default_target = default_target;
        self
    }

    pub fn with_default_timeout_ms(mut self, default_timeout_ms: u64) -> Self {
        self.default_timeout_ms = default_timeout_ms;
        self
    }

    pub fn handle_message(&self, message: Value) -> Result<Option<Value>, serde_json::Error> {
        if let Some(batch) = message.as_array() {
            return self.handle_batch(batch);
        }

        self.handle_single(message, false)
    }

    fn handle_batch(&self, batch: &[Value]) -> Result<Option<Value>, serde_json::Error> {
        if batch.is_empty() {
            return Ok(Some(error_response(
                Value::Null,
                INVALID_REQUEST,
                "batch must contain at least one request",
                None,
            )));
        }

        let mut responses = Vec::new();
        for message in batch {
            if let Some(response) = self.handle_single(message.clone(), true)? {
                responses.push(response);
            }
        }

        if responses.is_empty() {
            return Ok(None);
        }

        Ok(Some(Value::Array(responses)))
    }

    fn handle_single(
        &self,
        message: Value,
        in_batch: bool,
    ) -> Result<Option<Value>, serde_json::Error> {
        let request = match serde_json::from_value::<JsonRpcEnvelope>(message.clone()) {
            Ok(request) => request,
            Err(_) => {
                return Ok(Some(error_response(
                    message.get("id").cloned().unwrap_or(Value::Null),
                    INVALID_REQUEST,
                    "request is invalid",
                    None,
                )));
            }
        };

        if request.jsonrpc != JSONRPC_VERSION {
            return Ok(request.id.map(|id| {
                error_response(id, INVALID_REQUEST, "jsonrpc version must be 2.0", None)
            }));
        }

        let id = request.id;
        let response = match request.method.as_str() {
            "initialize" => {
                if in_batch {
                    id.map(|id| {
                        error_response(
                            id,
                            INVALID_REQUEST,
                            "initialize request must not be batched",
                            None,
                        )
                    })
                } else {
                    self.handle_initialize(id, request.params)?
                }
            }
            "notifications/initialized" => {
                if self.is_initialized() {
                    self.mark_ready();
                }
                None
            }
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tools_call(id, request.params)?,
            "ping" => id.map(|id| success_response(id, json!({}))),
            _ => id.map(|id| {
                error_response(
                    id,
                    METHOD_NOT_FOUND,
                    "method is not supported by this server",
                    None,
                )
            }),
        };

        Ok(response)
    }

    fn handle_initialize(
        &self,
        id: Option<Value>,
        params: Option<Value>,
    ) -> Result<Option<Value>, serde_json::Error> {
        let Some(id) = id else {
            return Ok(None);
        };

        let params = match serde_json::from_value::<InitializeParams>(params.unwrap_or_default()) {
            Ok(params) => params,
            Err(_) => {
                return Ok(Some(error_response(
                    id,
                    INVALID_PARAMS,
                    "initialize params are invalid",
                    None,
                )));
            }
        };
        let Some(protocol_version) = negotiate_protocol_version(&params.protocol_version) else {
            return Ok(Some(error_response(
                id,
                INVALID_PARAMS,
                "Unsupported protocol version",
                Some(json!({
                    "supported": SUPPORTED_PROTOCOL_VERSIONS,
                    "requested": params.protocol_version,
                })),
            )));
        };

        let mut state = self.state.lock().expect("server state poisoned");
        state.negotiated_protocol_version = Some(protocol_version);
        state.ready = false;

        Ok(Some(success_response(
            id,
            json!({
                "protocolVersion": protocol_version,
                "capabilities": {
                    "tools": {
                        "listChanged": false,
                    }
                },
                "serverInfo": {
                    "name": "operator-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        )))
    }

    fn handle_tools_list(&self, id: Option<Value>) -> Option<Value> {
        let id = id?;
        if !self.is_ready() {
            return Some(error_response(
                id,
                SERVER_NOT_INITIALIZED,
                "server not initialized",
                None,
            ));
        }

        Some(success_response(
            id,
            json!({
                "tools": self
                    .tools
                    .specs()
                    .iter()
                    .map(ToolDescriptor::from)
                    .collect::<Vec<_>>(),
            }),
        ))
    }

    fn handle_tools_call(
        &self,
        id: Option<Value>,
        params: Option<Value>,
    ) -> Result<Option<Value>, serde_json::Error> {
        let Some(id) = id else {
            return Ok(None);
        };

        if !self.is_ready() {
            return Ok(Some(error_response(
                id,
                SERVER_NOT_INITIALIZED,
                "server not initialized",
                None,
            )));
        }

        let params = match serde_json::from_value::<CallToolParams>(params.unwrap_or_default()) {
            Ok(params) => params,
            Err(_) => {
                return Ok(Some(error_response(
                    id,
                    INVALID_PARAMS,
                    "tools/call params are invalid",
                    None,
                )));
            }
        };

        if params.task.is_some() {
            return Ok(Some(error_response(
                id,
                INVALID_PARAMS,
                "task-augmented tool calls are not supported",
                None,
            )));
        }

        if !self
            .tools
            .specs()
            .iter()
            .any(|spec| spec.name == params.name)
        {
            return Ok(Some(error_response(
                id,
                INVALID_PARAMS,
                "tool is not registered",
                None,
            )));
        }

        let spec = self
            .find_tool_spec(&params.name)
            .ok_or_else(|| serde_json::Error::io(std::io::Error::other("missing tool spec")))?;
        let input = Value::Object(params.arguments);

        let result = match self.execute_tool_call(&spec, &params.name, input) {
            Ok(output) => call_tool_success(output)?,
            Err(error) => call_tool_error(error),
        };

        Ok(Some(success_response(id, result)))
    }

    fn execute_tool_call(
        &self,
        spec: &ToolSpec,
        name: &str,
        input: Value,
    ) -> Result<Value, OperatorError> {
        let exec_context = match serde_json::from_value::<ExecContextInput>(input.clone()) {
            Ok(exec_context) => exec_context,
            Err(_) => return self.invoke_tool(name, input),
        };

        if spec.has_side_effects && !self.allow_side_effects {
            return Err(OperatorError::Tool {
                tool: name.to_string(),
                message: "side effects are disabled by runtime policy".into(),
            });
        }

        let target = exec_context
            .target
            .unwrap_or_else(|| self.default_target.clone());
        let timeout_ms = exec_context.timeout_ms.unwrap_or(self.default_timeout_ms);
        let serializer = self.target_serializer(target);

        self.block_on(async move {
            let _permit = acquire_target_permit(serializer, timeout_ms).await?;
            self.tools.invoke(name, input).await
        })
    }

    fn invoke_tool(&self, name: &str, input: Value) -> Result<Value, OperatorError> {
        self.block_on(self.tools.invoke(name, input))
    }

    fn block_on<F>(&self, future: F) -> Result<Value, OperatorError>
    where
        F: Future<Output = Result<Value, OperatorError>> + Send,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::MultiThread => {
                    tokio::task::block_in_place(|| handle.block_on(future))
                }
                _ => Err(OperatorError::Platform(
                    "tools/call requires a multi-thread Tokio runtime".into(),
                )),
            },
            Err(_) => tokio::runtime::Runtime::new()
                .map_err(|error| {
                    OperatorError::Platform(format!("failed to start async runtime: {error}"))
                })?
                .block_on(future),
        }
    }

    fn is_initialized(&self) -> bool {
        self.state
            .lock()
            .expect("server state poisoned")
            .negotiated_protocol_version
            .is_some()
    }

    fn is_ready(&self) -> bool {
        self.state.lock().expect("server state poisoned").ready
    }

    fn mark_ready(&self) {
        self.state.lock().expect("server state poisoned").ready = true;
    }

    fn find_tool_spec(&self, name: &str) -> Option<ToolSpec> {
        self.tools
            .specs()
            .into_iter()
            .find(|spec| spec.name == name)
    }

    fn target_serializer(&self, target: TargetId) -> Arc<Semaphore> {
        let mut serializers = self
            .target_serializers
            .lock()
            .expect("target serializers poisoned");
        serializers
            .entry(target)
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }
}

pub(crate) fn parse_error_response() -> Value {
    error_response(
        Value::Null,
        PARSE_ERROR,
        "failed to parse JSON-RPC message",
        None,
    )
}

fn negotiate_protocol_version(requested: &str) -> Option<&'static str> {
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .copied()
        .find(|version| *version == requested)
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result,
    })
}

fn error_response(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({
        "code": code,
        "message": message,
    });
    if let Some(data) = data {
        error["data"] = data;
    }

    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": error,
    })
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
}

#[derive(Debug, Deserialize)]
struct CallToolParams {
    #[allow(dead_code)]
    #[serde(default)]
    task: Option<Value>,
    name: String,
    #[serde(default)]
    arguments: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ExecContextInput {
    target: Option<TargetId>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct ServerState {
    negotiated_protocol_version: Option<&'static str>,
    ready: bool,
}

#[derive(Debug, Serialize)]
struct ToolDescriptor<'a> {
    name: &'a str,
    description: &'a str,
    #[serde(rename = "inputSchema")]
    input_schema: &'a Value,
    #[serde(rename = "outputSchema")]
    output_schema: &'a Value,
    annotations: ToolAnnotations,
}

impl<'a> From<&'a ToolSpec> for ToolDescriptor<'a> {
    fn from(spec: &'a ToolSpec) -> Self {
        Self {
            name: spec.name,
            description: spec.description,
            input_schema: &spec.input_schema,
            output_schema: &spec.output_schema,
            annotations: ToolAnnotations {
                read_only_hint: !spec.has_side_effects,
                destructive_hint: spec.has_side_effects,
                open_world_hint: false,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ToolAnnotations {
    #[serde(rename = "readOnlyHint")]
    read_only_hint: bool,
    #[serde(rename = "destructiveHint")]
    destructive_hint: bool,
    #[serde(rename = "openWorldHint")]
    open_world_hint: bool,
}

fn call_tool_success(output: Value) -> Result<Value, serde_json::Error> {
    let content = serde_json::to_string_pretty(&output)?;
    let mut result = json!({
        "content": [
            text_content(content),
        ],
    });

    if let Value::Object(object) = output {
        result["structuredContent"] = Value::Object(object);
    }

    Ok(result)
}

fn call_tool_error(error: OperatorError) -> Value {
    json!({
        "content": [
            text_content(error.to_string()),
        ],
        "isError": true,
    })
}

fn text_content(text: String) -> Value {
    json!({
        "type": "text",
        "text": text,
    })
}

async fn acquire_target_permit(
    serializer: Arc<Semaphore>,
    timeout_ms: u64,
) -> Result<OwnedSemaphorePermit, OperatorError> {
    time::timeout(
        Duration::from_millis(timeout_ms),
        serializer.acquire_owned(),
    )
    .await
    .map_err(|_| OperatorError::TargetBusy)?
    .map_err(|_| OperatorError::Platform("target serializer closed".into()))
}
