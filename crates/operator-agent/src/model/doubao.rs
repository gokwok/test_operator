use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

use reqwest::Client;
use serde_json::{json, Map, Value};

use super::{
    channel, AssistantMessage, CallOptions, ContentBlock, Context, DoneReason, ErrorReason,
    Message, ModelError, ModelEvent, ModelProvider, ModelRequest, ModelStream, ModelStreamWriter,
    ProviderKind, ReasoningLevel, ResponseFormat, StopReason, ToolResultMessage, ToolSpec, Usage,
};

const DEFAULT_BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/v3";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoubaoProviderConfig {
    pub api_key: String,
    pub base_url: String,
}

impl DoubaoProviderConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct DoubaoChatCompletionsProvider {
    client: Client,
    config: DoubaoProviderConfig,
}

impl DoubaoChatCompletionsProvider {
    pub fn new(config: DoubaoProviderConfig) -> Result<Self, ModelError> {
        let client = Client::builder()
            .build()
            .map_err(|error| ModelError::ProviderInitFailed {
                provider: ProviderKind::OpenAiCompatible,
                message: error.to_string(),
            })?;

        Self::with_client(config, client)
    }

    pub fn with_client(config: DoubaoProviderConfig, client: Client) -> Result<Self, ModelError> {
        if config.api_key.trim().is_empty() {
            return Err(ModelError::ProviderInitFailed {
                provider: ProviderKind::OpenAiCompatible,
                message: "api key must not be empty".into(),
            });
        }

        let mut config = config;
        if config.base_url.trim().is_empty() {
            config.base_url = DEFAULT_BASE_URL.to_string();
        }

        Ok(Self { client, config })
    }

    async fn execute(&self, req: ModelRequest) -> Result<AssistantMessage, ModelError> {
        let endpoint = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
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
            ModelError::Protocol(format!("doubao response was not valid json: {error}"))
        })?;

        assistant_message_from_response(payload)
    }
}

impl ModelProvider for DoubaoChatCompletionsProvider {
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
    body.insert("messages".into(), Value::Array(messages(context)));

    if let Some(reasoning_level) = options.reasoning_level {
        body.insert(
            "reasoning_effort".into(),
            Value::String(reasoning_effort(reasoning_level).to_string()),
        );
    }
    if let Some(temperature) = options.temperature {
        body.insert("temperature".into(), json!(temperature));
    }
    if let Some(max_output_tokens) = options.max_output_tokens {
        body.insert("max_tokens".into(), json!(max_output_tokens));
    }
    if let Some(response_format) = options.response_format {
        body.insert(
            "response_format".into(),
            response_format_payload(response_format),
        );
    }

    Value::Object(body)
}

fn messages(context: &Context) -> Vec<Value> {
    let mut messages = Vec::new();
    if let Some(instructions) = instructions(context.system.as_deref(), &context.tools) {
        messages.push(json!({
            "role": "system",
            "content": instructions,
        }));
    }
    messages.extend(context.messages.iter().map(message_input));
    messages
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

fn message_input(message: &Message) -> Value {
    match message {
        Message::User(message) => message_with_role("user", &message.content),
        Message::Assistant(message) => message_with_role("assistant", &message.content),
        Message::ToolResult(ToolResultMessage { content, .. }) => {
            message_with_role("user", content)
        }
    }
}

fn message_with_role(role: &str, content: &[ContentBlock]) -> Value {
    json!({
        "role": role,
        "content": content.iter().filter_map(input_content_item).collect::<Vec<_>>(),
    })
}

fn input_content_item(block: &ContentBlock) -> Option<Value> {
    match block {
        ContentBlock::Text { text } => Some(json!({
            "type": "text",
            "text": text,
        })),
        ContentBlock::Image { mime, data_base64 } => Some(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{mime};base64,{data_base64}"),
            },
        })),
        ContentBlock::Thinking { thinking } => Some(json!({
            "type": "text",
            "text": thinking,
        })),
        ContentBlock::ToolCall {
            id,
            name,
            arguments_json,
        } => Some(json!({
            "type": "text",
            "text": serde_json::to_string_pretty(&json!({
                "tool_call_id": id,
                "tool_name": name,
                "arguments_json": arguments_json,
            }))
            .expect("tool call blocks should serialize to json"),
        })),
    }
}

fn reasoning_effort(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Minimal => "minimal",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
    }
}

fn response_format_payload(format: ResponseFormat) -> Value {
    match format {
        ResponseFormat::JsonObject => json!({ "type": "json_object" }),
    }
}

fn assistant_message_from_response(response: Value) -> Result<AssistantMessage, ModelError> {
    let choices = response
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| ModelError::Protocol("doubao response missing `choices` array".into()))?;

    let choice = choices
        .first()
        .ok_or_else(|| ModelError::Protocol("doubao response did not include a choice".into()))?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| ModelError::Protocol("doubao response missing choice `message`".into()))?;

    let mut content = Vec::new();
    if let Some(reasoning) = message.get("reasoning_content").and_then(Value::as_str) {
        if !reasoning.trim().is_empty() {
            content.push(ContentBlock::Thinking {
                thinking: reasoning.to_string(),
            });
        }
    }
    append_message_blocks(&mut content, message.get("content"));

    if !content
        .iter()
        .any(|block| matches!(block, ContentBlock::Text { .. }))
    {
        return Err(ModelError::Protocol(
            "doubao response missing assistant text content".into(),
        ));
    }

    Ok(AssistantMessage {
        content,
        usage: usage_from_response(&response),
        stop: stop_reason_from_response(choice),
        error_message: None,
        timestamp_ms: timestamp_ms(),
    })
}

fn append_message_blocks(content: &mut Vec<ContentBlock>, value: Option<&Value>) {
    let Some(value) = value else {
        return;
    };

    match value {
        Value::String(text) => {
            content.push(ContentBlock::Text { text: text.clone() });
        }
        Value::Array(blocks) => {
            for block in blocks {
                match block {
                    Value::String(text) => content.push(ContentBlock::Text { text: text.clone() }),
                    Value::Object(object) => {
                        let block_type = object.get("type").and_then(Value::as_str);
                        let text = object.get("text").and_then(Value::as_str);
                        if matches!(block_type, Some("text") | Some("output_text")) {
                            if let Some(text) = text {
                                content.push(ContentBlock::Text {
                                    text: text.to_string(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn usage_from_response(response: &Value) -> Usage {
    let Some(usage) = response.get("usage").and_then(Value::as_object) else {
        return Usage::default();
    };

    Usage {
        input_tokens: usage
            .get("prompt_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        total_tokens: usage
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        cost: None,
    }
}

fn stop_reason_from_response(choice: &Value) -> StopReason {
    match choice.get("finish_reason").and_then(Value::as_str) {
        Some("length") => StopReason::Length,
        Some("tool_calls") | Some("tool_use") => StopReason::ToolUse,
        Some("content_filter") => StopReason::Error,
        Some("stop") | None => StopReason::Stop,
        _ => StopReason::Stop,
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
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| String::from_utf8_lossy(body).trim().to_string());

    ModelError::Provider(format!(
        "doubao chat completions api returned {status}: {parsed}"
    ))
}

fn done_reason(stop: StopReason) -> DoneReason {
    match stop {
        StopReason::Stop => DoneReason::Stop,
        StopReason::Length => DoneReason::Length,
        StopReason::ToolUse => DoneReason::ToolUse,
        StopReason::Aborted | StopReason::Error => DoneReason::Stop,
    }
}

fn error_reason(error: &ModelError) -> ErrorReason {
    match error {
        ModelError::Aborted => ErrorReason::Aborted,
        ModelError::Timeout
        | ModelError::ModelNotFound(_)
        | ModelError::ProviderNotFound { .. }
        | ModelError::ProviderInitFailed { .. }
        | ModelError::Transport(_)
        | ModelError::Protocol(_)
        | ModelError::Provider(_) => ErrorReason::Error,
    }
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
