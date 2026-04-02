use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

use operator_agent::model::{
    ApiKind, CallOptions, ContentBlock, Context, CoordinatePolicy, HttpProviderConfig, Message,
    ModelConfig, ModelError, ModelEvent, ModelProvider, ProviderKind, ReasoningLevel,
    ResponseFormat, ResponsesProvider, ToolSpec, UserMessage,
};
use serde_json::{json, Value};

fn provider(base_url: String) -> ResponsesProvider {
    ResponsesProvider::new(HttpProviderConfig {
        provider: ProviderKind::OpenAi,
        api_key: "test-key".into(),
        base_url,
    })
    .expect("provider should build")
}

fn model_config() -> ModelConfig {
    ModelConfig {
        provider: ProviderKind::OpenAi,
        api_kind: ApiKind::Responses,
        id: "gpt-5.4".into(),
        coordinate_policy: CoordinatePolicy::SurfaceImagePixels,
        default_options: CallOptions::default(),
        default_timeout_ms: Some(30_000),
    }
}

#[tokio::test]
async fn openai_provider_uses_responses_api_for_json_planner_requests() {
    let server = MockServer::spawn(
        200,
        json!({
            "id": "resp_123",
            "status": "completed",
            "output": [
                {
                    "id": "rs_123",
                    "type": "reasoning",
                    "summary": [],
                    "content": []
                },
                {
                    "id": "msg_123",
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "annotations": [],
                            "logprobs": [],
                            "text": "{\"decision\":\"finish\",\"summary\":\"The UI is confirmed.\"}"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 42,
                "output_tokens": 9,
                "total_tokens": 51
            }
        }),
        Duration::from_millis(0),
    );
    let provider = provider(server.base_url());

    let image_base64 = "bm90LWEtcmVhbC1wbmc=";
    let request = operator_agent::model::ModelRequest {
        config: model_config(),
        context: Context {
            system: Some("You are the Operator planner.".into()),
            messages: vec![Message::User(UserMessage {
                content: vec![
                    ContentBlock::Text {
                        text: "Inspect the screenshot and finish in JSON.".into(),
                    },
                    ContentBlock::Image {
                        mime: "image/png".into(),
                        data_base64: image_base64.into(),
                    },
                ],
                timestamp_ms: 0,
            })],
            tools: vec![ToolSpec {
                name: "observe".into(),
                description: "Capture the frontmost UI surface.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "surface": { "type": "string" }
                    }
                }),
            }],
        },
        options: CallOptions {
            max_output_tokens: Some(512),
            reasoning_level: Some(ReasoningLevel::Minimal),
            response_format: Some(ResponseFormat::JsonObject),
            ..CallOptions::default()
        },
        stream: false,
        timeout: Some(Duration::from_secs(5)),
        request_id: Some("req-openai-1".into()),
        max_retry_delay_ms: None,
    };

    let mut stream = provider.stream(request);
    let mut events = Vec::new();
    while let Some(event) = stream.recv().await {
        let done = matches!(event, ModelEvent::Done { .. } | ModelEvent::Error { .. });
        events.push(event);
        if done {
            break;
        }
    }

    let message = stream.result().await.expect("provider should return text");
    let recorded = server.recorded_request();

    assert_eq!(recorded.method, "POST");
    assert_eq!(recorded.path, "/v1/responses");
    assert_eq!(
        recorded
            .headers
            .get("authorization")
            .expect("authorization header should exist"),
        "Bearer test-key"
    );
    assert_eq!(recorded.body["model"], Value::String("gpt-5.4".into()));
    assert_eq!(
        recorded.body["reasoning"]["effort"],
        Value::String("none".into())
    );
    assert_eq!(
        recorded.body["text"]["format"]["type"],
        Value::String("json_object".into())
    );
    assert_eq!(
        recorded.body["max_output_tokens"],
        Value::Number(512.into())
    );
    assert!(
        recorded.body["instructions"]
            .as_str()
            .expect("instructions should be a string")
            .contains("observe"),
        "tool catalog should be embedded in instructions: {}",
        recorded.body["instructions"]
    );
    assert_eq!(
        recorded.body["input"][0]["content"][1]["type"],
        Value::String("input_image".into())
    );
    assert_eq!(
        recorded.body["input"][0]["content"][1]["image_url"],
        Value::String(format!("data:image/png;base64,{image_base64}"))
    );

    assert_eq!(
        message.content,
        vec![ContentBlock::Text {
            text: "{\"decision\":\"finish\",\"summary\":\"The UI is confirmed.\"}".into(),
        }]
    );
    assert_eq!(message.usage.input_tokens, 42);
    assert_eq!(message.usage.output_tokens, 9);
    assert_eq!(message.usage.total_tokens, 51);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ModelEvent::Start)),
        "provider should emit Start before completion: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            ModelEvent::TextDelta { delta, .. }
                if delta.contains("\"decision\":\"finish\"")
        )),
        "provider should emit text deltas for the assistant message: {events:?}"
    );
}

#[tokio::test]
async fn openai_provider_maps_request_timeout_to_model_timeout() {
    let server = MockServer::spawn(
        200,
        json!({
            "id": "resp_timeout",
            "status": "completed",
            "output": [
                {
                    "id": "msg_timeout",
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "{\"decision\":\"finish\",\"summary\":\"too late\"}",
                            "annotations": [],
                            "logprobs": []
                        }
                    ]
                }
            ]
        }),
        Duration::from_millis(200),
    );
    let provider = provider(server.base_url());

    let request = operator_agent::model::ModelRequest {
        config: model_config(),
        context: Context {
            system: Some("You are the Operator planner.".into()),
            messages: vec![Message::User(UserMessage {
                content: vec![ContentBlock::Text {
                    text: "Finish quickly.".into(),
                }],
                timestamp_ms: 0,
            })],
            tools: Vec::new(),
        },
        options: CallOptions::default(),
        stream: false,
        timeout: Some(Duration::from_millis(50)),
        request_id: None,
        max_retry_delay_ms: None,
    };

    let mut stream = provider.stream(request);
    let mut saw_timeout_error = false;
    while let Some(event) = stream.recv().await {
        if matches!(
            event,
            ModelEvent::Error {
                error: ModelError::Timeout,
                ..
            }
        ) {
            saw_timeout_error = true;
            break;
        }
    }

    assert!(
        matches!(stream.result().await, Err(ModelError::Timeout)),
        "provider should map request timeout to ModelError::Timeout"
    );
    assert!(
        saw_timeout_error,
        "timeout should be surfaced in the event stream"
    );
}

#[tokio::test]
async fn openai_provider_preserves_multiple_image_blocks_in_order() {
    let server = MockServer::spawn(
        200,
        json!({
            "id": "resp_images",
            "status": "completed",
            "output": [
                {
                    "id": "msg_images",
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "annotations": [],
                            "logprobs": [],
                            "text": "{\"decision\":\"finish\",\"summary\":\"Images loaded.\"}"
                        }
                    ]
                }
            ]
        }),
        Duration::from_millis(0),
    );
    let provider = provider(server.base_url());

    let request = operator_agent::model::ModelRequest {
        config: model_config(),
        context: Context {
            system: None,
            messages: vec![Message::User(UserMessage {
                content: vec![
                    ContentBlock::Text {
                        text: "Compare both screenshots.".into(),
                    },
                    ContentBlock::Image {
                        mime: "image/png".into(),
                        data_base64: "cHJldmlvdXM=".into(),
                    },
                    ContentBlock::Image {
                        mime: "image/png".into(),
                        data_base64: "Y3VycmVudA==".into(),
                    },
                ],
                timestamp_ms: 0,
            })],
            tools: Vec::new(),
        },
        options: CallOptions::default(),
        stream: false,
        timeout: Some(Duration::from_secs(5)),
        request_id: None,
        max_retry_delay_ms: None,
    };

    let _ = provider
        .stream(request)
        .result()
        .await
        .expect("provider should succeed");
    let recorded = server.recorded_request();

    let content = recorded.body["input"][0]["content"]
        .as_array()
        .expect("input content should be an array");
    assert_eq!(content[1]["type"], Value::String("input_image".into()));
    assert_eq!(
        content[1]["image_url"],
        Value::String("data:image/png;base64,cHJldmlvdXM=".into())
    );
    assert_eq!(content[2]["type"], Value::String("input_image".into()));
    assert_eq!(
        content[2]["image_url"],
        Value::String("data:image/png;base64,Y3VycmVudA==".into())
    );
}

#[tokio::test]
async fn openai_provider_encodes_assistant_history_as_output_text() {
    let server = MockServer::spawn(
        200,
        json!({
            "id": "resp_history",
            "status": "completed",
            "output": [
                {
                    "id": "msg_history",
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "{\"decision\":\"finish\",\"summary\":\"confirmed\"}",
                            "annotations": [],
                            "logprobs": []
                        }
                    ]
                }
            ]
        }),
        Duration::from_millis(0),
    );
    let provider = provider(server.base_url());

    let request = operator_agent::model::ModelRequest {
        config: model_config(),
        context: Context {
            system: Some("You are the Operator planner.".into()),
            messages: vec![
                Message::User(UserMessage {
                    content: vec![ContentBlock::Text {
                        text: "Plan in JSON.".into(),
                    }],
                    timestamp_ms: 0,
                }),
                Message::Assistant(operator_agent::model::AssistantMessage {
                    content: vec![ContentBlock::Text {
                        text: "{\"decision\":\"call_tool\",\"name\":\"type\"}".into(),
                    }],
                    usage: Default::default(),
                    stop: operator_agent::model::StopReason::Stop,
                    error_message: None,
                    timestamp_ms: 1,
                }),
                Message::ToolResult(operator_agent::model::ToolResultMessage {
                    tool_call_id: "tool-1".into(),
                    tool_name: "type".into(),
                    content: vec![ContentBlock::Text {
                        text: "{\"success\":true}".into(),
                    }],
                    is_error: false,
                    timestamp_ms: 2,
                }),
            ],
            tools: Vec::new(),
        },
        options: CallOptions::default(),
        stream: false,
        timeout: Some(Duration::from_secs(5)),
        request_id: Some("req-openai-history".into()),
        max_retry_delay_ms: None,
    };

    let mut stream = provider.stream(request);
    while let Some(event) = stream.recv().await {
        if matches!(event, ModelEvent::Done { .. } | ModelEvent::Error { .. }) {
            break;
        }
    }

    let _message = stream.result().await.expect("provider should return text");
    let recorded = server.recorded_request();

    assert_eq!(
        recorded.body["input"][0]["role"],
        Value::String("user".into())
    );
    assert_eq!(
        recorded.body["input"][0]["content"][0]["type"],
        Value::String("input_text".into())
    );
    assert_eq!(
        recorded.body["input"][1]["role"],
        Value::String("assistant".into())
    );
    assert_eq!(
        recorded.body["input"][1]["content"][0]["type"],
        Value::String("output_text".into())
    );
    assert_eq!(
        recorded.body["input"][2]["type"],
        Value::String("function_call_output".into())
    );
    assert_eq!(
        recorded.body["input"][2]["call_id"],
        Value::String("tool-1".into())
    );
}

#[derive(Debug)]
struct RecordedRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

struct MockServer {
    addr: SocketAddr,
    request_rx: mpsc::Receiver<RecordedRequest>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MockServer {
    fn spawn(status: u16, body: Value, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should have an address");
        let (request_tx, request_rx) = mpsc::channel();

        let thread = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("request should connect");
            let recorded = read_request(stream.try_clone().expect("stream should clone"));
            request_tx
                .send(recorded)
                .expect("recorded request should send");
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            write_response(stream, status, &body.to_string());
        });

        Self {
            addr,
            request_rx,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    fn recorded_request(&self) -> RecordedRequest {
        self.request_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("request should be recorded")
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_request(stream: TcpStream) -> RecordedRequest {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .expect("request line should read");

    let mut headers = HashMap::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("header should read");
        if line == "\r\n" {
            break;
        }

        let (name, value) = line.split_once(':').expect("header should contain a colon");
        let value = value.trim().to_string();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value
                .parse::<usize>()
                .expect("content-length should be numeric");
        }
        headers.insert(name.to_ascii_lowercase(), value);
    }

    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .expect("request body should read fully");
    let body = serde_json::from_slice(&body).expect("request body should be valid json");

    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("request should include a method");
    let path = parts.next().expect("request should include a path");

    RecordedRequest {
        method: method.to_string(),
        path: path.to_string(),
        headers,
        body,
    }
}

fn write_response(mut stream: TcpStream, status: u16, body: &str) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        500 => "Internal Server Error",
        other => panic!("unsupported status code in test server: {other}"),
    };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("response should write");
    stream.flush().expect("response should flush");
}
