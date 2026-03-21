use operator_core::OperatorError;
use operator_runtime::{ToolRegistry, ToolSpec};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Map, Value};

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
    negotiated_protocol_version: Option<&'static str>,
    ready: bool,
}

impl McpServer {
    pub fn new(tools: ToolRegistry) -> Self {
        Self {
            tools,
            negotiated_protocol_version: None,
            ready: false,
        }
    }

    pub fn handle_message(&mut self, message: Value) -> Result<Option<Value>, serde_json::Error> {
        if let Some(batch) = message.as_array() {
            return self.handle_batch(batch);
        }

        self.handle_single(message, false)
    }

    fn handle_batch(&mut self, batch: &[Value]) -> Result<Option<Value>, serde_json::Error> {
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
        &mut self,
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
                if self.negotiated_protocol_version.is_some() {
                    self.ready = true;
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
        &mut self,
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

        self.negotiated_protocol_version = Some(protocol_version);
        self.ready = false;

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
        if !self.ready {
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

        if !self.ready {
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

        let result = match self.invoke_tool(&params.name, Value::Object(params.arguments)) {
            Ok(output) => call_tool_success(output)?,
            Err(error) => call_tool_error(error),
        };

        Ok(Some(success_response(id, result)))
    }

    fn invoke_tool(&self, name: &str, input: Value) -> Result<Value, OperatorError> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::MultiThread => {
                    tokio::task::block_in_place(|| handle.block_on(self.tools.invoke(name, input)))
                }
                _ => Err(OperatorError::Platform(
                    "tools/call requires a multi-thread Tokio runtime".into(),
                )),
            },
            Err(_) => tokio::runtime::Runtime::new()
                .map_err(|error| {
                    OperatorError::Platform(format!("failed to start async runtime: {error}"))
                })?
                .block_on(self.tools.invoke(name, input)),
        }
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
