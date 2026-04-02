use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

use reqwest::Client;
use serde_json::{json, Map, Value};

use super::{
    channel, AssistantMessage, CallOptions, ContentBlock, Context, DoneReason, ErrorReason,
    HttpProviderConfig, Message, ModelError, ModelEvent, ModelProvider, ModelRequest, ModelStream,
    ModelStreamWriter, ReasoningLevel, ResponseFormat, StopReason, ToolResultMessage, ToolSpec,
    Usage,
};

#[derive(Clone)]
pub struct ResponsesProvider {
    client: Client,
    config: HttpProviderConfig,
}

impl ResponsesProvider {
    pub fn new(config: HttpProviderConfig) -> Result<Self, ModelError> {
        let provider = config.provider;
        let client = Client::builder()
            .build()
            .map_err(|error| ModelError::ProviderInitFailed {
                provider,
                message: error.to_string(),
            })?;

        Self::with_client(config, client)
    }

    pub fn with_client(config: HttpProviderConfig, client: Client) -> Result<Self, ModelError> {
        if config.api_key.trim().is_empty() {
            return Err(ModelError::ProviderInitFailed {
                provider: config.provider,
                message: "api key must not be empty".into(),
            });
        }
        if config.base_url.trim().is_empty() {
            return Err(ModelError::ProviderInitFailed {
                provider: config.provider,
                message: "base_url must not be empty".into(),
            });
        }

        Ok(Self { client, config })
    }

    async fn execute(&self, req: ModelRequest) -> Result<AssistantMessage, ModelError> {
        let endpoint = format!("{}/responses", self.config.base_url.trim_end_matches('/'));
        let payload = request_body(&req.context, &req.config.id, &req.options);

        let mut request = self
            .client
            .post(endpoint)
            .bearer_auth(&self.config.api_key)
            .json(&payload);
        if let Some(timeout) = req.timeout {
            request = request.timeout(timeout);
        }

        let response = request.send().await.map_err(request_error)?;
        let status = response.status();
        let body = response.bytes().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(http_error(status.as_u16(), &body));
        }

        let payload = serde_json::from_slice::<Value>(&body).map_err(|error| {
            ModelError::Protocol(format!(
                "responses api response was not valid json: {error}"
            ))
        })?;

        assistant_message_from_response(payload)
    }
}

impl ModelProvider for ResponsesProvider {
    fn stream(&self, req: ModelRequest) -> ModelStream {
        let (stream, writer) = channel(NonZeroUsize::new(16).expect("event capacity must be set"));
        let provider = self.clone();

        tokio::spawn(async move {
            let _ = writer.emit(ModelEvent::Start).await;
            match provider.execute(req).await {
                Ok(message) => finish_success(writer, message).await,
                Err(error) => finish_error(writer, error).await,
            }
        });

        stream
    }
}

async fn finish_success(writer: ModelStreamWriter, message: AssistantMessage) {
    emit_content_events(&writer, &message.content).await;
    let _ = writer
        .emit(ModelEvent::Done {
            reason: done_reason(message.stop),
            message: message.clone(),
        })
        .await;
    let _ = writer.finish(Ok(message));
}

async fn finish_error(writer: ModelStreamWriter, error: ModelError) {
    let _ = writer
        .emit(ModelEvent::Error {
            reason: error_reason(&error),
            error: error.clone(),
        })
        .await;
    let _ = writer.finish(Err(error));
}

async fn emit_content_events(writer: &ModelStreamWriter, content: &[ContentBlock]) {
    for (index, block) in content.iter().enumerate() {
        match block {
            ContentBlock::Text { text } => {
                let _ = writer
                    .emit(ModelEvent::TextStart {
                        content_index: index,
                    })
                    .await;
                if !text.is_empty() {
                    let _ = writer
                        .emit(ModelEvent::TextDelta {
                            content_index: index,
                            delta: text.clone(),
                        })
                        .await;
                }
                let _ = writer
                    .emit(ModelEvent::TextEnd {
                        content_index: index,
                    })
                    .await;
            }
            ContentBlock::Thinking { thinking } => {
                let _ = writer
                    .emit(ModelEvent::ThinkingStart {
                        content_index: index,
                    })
                    .await;
                if !thinking.is_empty() {
                    let _ = writer
                        .emit(ModelEvent::ThinkingDelta {
                            content_index: index,
                            delta: thinking.clone(),
                        })
                        .await;
                }
                let _ = writer
                    .emit(ModelEvent::ThinkingEnd {
                        content_index: index,
                    })
                    .await;
            }
            ContentBlock::ToolCall {
                id,
                name,
                arguments_json,
            } => {
                let _ = writer
                    .emit(ModelEvent::ToolCallStart {
                        content_index: index,
                        tool_call_id: id.clone(),
                        tool_name: Some(name.clone()),
                    })
                    .await;
                if !arguments_json.is_empty() {
                    let _ = writer
                        .emit(ModelEvent::ToolCallDelta {
                            content_index: index,
                            tool_call_id: id.clone(),
                            tool_name: Some(name.clone()),
                            arguments_delta: arguments_json.clone(),
                        })
                        .await;
                }
                let _ = writer
                    .emit(ModelEvent::ToolCallEnd {
                        content_index: index,
                        tool_call_id: id.clone(),
                    })
                    .await;
            }
            ContentBlock::Image { .. } => {}
        }
    }
}

fn request_body(context: &Context, model: &Arc<str>, options: &CallOptions) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), Value::String(model.to_string()));
    body.insert(
        "input".into(),
        Value::Array(context.messages.iter().map(message_input).collect()),
    );
    body.insert("store".into(), Value::Bool(false));

    if let Some(instructions) = instructions(context.system.as_deref(), &context.tools) {
        body.insert("instructions".into(), Value::String(instructions));
    }
    if let Some(reasoning_level) = options.reasoning_level {
        body.insert(
            "reasoning".into(),
            json!({
                "effort": reasoning_effort(reasoning_level),
            }),
        );
    }
    if let Some(temperature) = options.temperature {
        body.insert("temperature".into(), json!(temperature));
    }
    if let Some(max_output_tokens) = options.max_output_tokens {
        body.insert("max_output_tokens".into(), json!(max_output_tokens));
    }
    if let Some(response_format) = options.response_format {
        body.insert(
            "text".into(),
            json!({
                "format": response_format_payload(response_format),
            }),
        );
    }

    Value::Object(body)
}

fn instructions(system: Option<&str>, tools: &[ToolSpec]) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(system) = system {
        let system = system.trim();
        if !system.is_empty() {
            parts.push(system.to_string());
        }
    }
    if !tools.is_empty() {
        let catalog = serde_json::to_string_pretty(tools)
            .expect("tool catalog should always serialize to json");
        parts.push(format!(
            "Available tools (planning reference only; do not use provider-native tool calling):\n{catalog}"
        ));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn response_format_payload(response_format: ResponseFormat) -> Value {
    match response_format {
        ResponseFormat::JsonObject => json!({
            "type": "json_object",
        }),
    }
}

fn reasoning_effort(reasoning_level: ReasoningLevel) -> &'static str {
    match reasoning_level {
        ReasoningLevel::Minimal => "none",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
    }
}

fn message_input(message: &Message) -> Value {
    match message {
        Message::User(message) => json!({
            "role": "user",
            "content": message.content.iter().map(content_input).collect::<Vec<_>>(),
        }),
        Message::Assistant(message) => json!({
            "role": "assistant",
            "content": assistant_content(message.content.as_slice()),
        }),
        Message::ToolResult(message) => tool_result_input(message),
    }
}

fn content_input(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { text } => json!({
            "type": "input_text",
            "text": text,
        }),
        ContentBlock::Image { mime, data_base64 } => json!({
            "type": "input_image",
            "image_url": format!("data:{mime};base64,{data_base64}"),
        }),
        ContentBlock::Thinking { thinking } => json!({
            "type": "input_text",
            "text": thinking,
        }),
        ContentBlock::ToolCall {
            id,
            name,
            arguments_json,
        } => json!({
            "type": "function_call",
            "call_id": id,
            "name": name,
            "arguments": arguments_json,
        }),
    }
}

fn assistant_content(content: &[ContentBlock]) -> Vec<Value> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(json!({
                "type": "output_text",
                "text": text,
            })),
            ContentBlock::Thinking { thinking } => Some(json!({
                "type": "output_text",
                "text": thinking,
            })),
            ContentBlock::ToolCall {
                id,
                name,
                arguments_json,
            } => Some(json!({
                "type": "function_call",
                "call_id": id,
                "name": name,
                "arguments": arguments_json,
            })),
            ContentBlock::Image { .. } => None,
        })
        .collect()
}

fn tool_result_input(message: &ToolResultMessage) -> Value {
    let output = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(Value::String(text.clone())),
            ContentBlock::Thinking { thinking } => Some(Value::String(thinking.clone())),
            ContentBlock::Image { .. } | ContentBlock::ToolCall { .. } => None,
        })
        .collect::<Vec<_>>();
    json!({
        "type": "function_call_output",
        "call_id": message.tool_call_id,
        "output": output,
    })
}

fn assistant_message_from_response(payload: Value) -> Result<AssistantMessage, ModelError> {
    let output = payload
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ModelError::Protocol("responses api response missing `output` array".into())
        })?;

    let mut content = Vec::new();
    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("reasoning") => {}
            Some("message") => {
                let message_content =
                    item.get("content")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            ModelError::Protocol(
                                "responses api assistant message missing `content` array".into(),
                            )
                        })?;
                for block in message_content {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        content.push(ContentBlock::Text {
                            text: text.to_string(),
                        });
                    }
                }
            }
            Some("function_call") => {
                let id = item.get("call_id").and_then(Value::as_str).ok_or_else(|| {
                    ModelError::Protocol("responses api function_call missing `call_id`".into())
                })?;
                let name = item.get("name").and_then(Value::as_str).ok_or_else(|| {
                    ModelError::Protocol("responses api function_call missing `name`".into())
                })?;
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ModelError::Protocol(
                            "responses api function_call missing `arguments`".into(),
                        )
                    })?;
                content.push(ContentBlock::ToolCall {
                    id: Arc::from(id),
                    name: Arc::from(name),
                    arguments_json: arguments.to_string(),
                });
            }
            _ => {}
        }
    }

    if content.is_empty() {
        return Err(ModelError::Protocol(
            "responses api response missing assistant `output_text` content".into(),
        ));
    }

    Ok(AssistantMessage {
        content,
        usage: usage_from_responses_payload(&payload),
        stop: stop_reason_from_response(&payload),
        error_message: None,
        timestamp_ms: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    })
}

fn usage_from_responses_payload(payload: &Value) -> Usage {
    let usage = payload.get("usage");
    Usage {
        input_tokens: usage
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        output_tokens: usage
            .and_then(|usage| usage.get("output_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        total_tokens: usage
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        cost: None,
    }
}

fn stop_reason_from_response(payload: &Value) -> StopReason {
    match payload.get("status").and_then(Value::as_str) {
        Some("completed") => StopReason::Stop,
        Some("incomplete") => StopReason::Length,
        Some("failed") => StopReason::Error,
        Some("cancelled") => StopReason::Aborted,
        _ => StopReason::Stop,
    }
}

fn done_reason(reason: StopReason) -> DoneReason {
    match reason {
        StopReason::Stop => DoneReason::Stop,
        StopReason::Length => DoneReason::Length,
        StopReason::ToolUse => DoneReason::ToolUse,
        StopReason::Aborted | StopReason::Error => DoneReason::Stop,
    }
}

fn error_reason(error: &ModelError) -> ErrorReason {
    match error {
        ModelError::Aborted => ErrorReason::Aborted,
        ModelError::ModelNotFound(_)
        | ModelError::Timeout
        | ModelError::ProviderNotFound { .. }
        | ModelError::ProviderInitFailed { .. }
        | ModelError::Transport(_)
        | ModelError::Protocol(_)
        | ModelError::Provider(_) => ErrorReason::Error,
    }
}

fn request_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else {
        ModelError::Transport(error.to_string())
    }
}

fn http_error(status: u16, body: &[u8]) -> ModelError {
    let parsed = serde_json::from_slice::<Value>(body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body).into_owned()));
    ModelError::Provider(format!("responses api returned {status}: {parsed}"))
}
