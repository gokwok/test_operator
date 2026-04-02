use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

use reqwest::Client;
use serde_json::{json, Map, Value};

use super::{
    channel, AssistantMessage, CallOptions, ContentBlock, Context, DoneReason, ErrorReason,
    HttpProviderConfig, Message, ModelError, ModelEvent, ModelProvider, ModelRequest, ModelStream,
    ModelStreamWriter, ReasoningLevel, ResponseFormat, StopReason, ToolSpec, Usage,
};

#[derive(Clone)]
pub struct ChatCompletionsProvider {
    client: Client,
    config: HttpProviderConfig,
}

impl ChatCompletionsProvider {
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
            ModelError::Protocol(format!(
                "chat completions response was not valid json: {error}"
            ))
        })?;

        assistant_message_from_response(payload)
    }
}

impl ModelProvider for ChatCompletionsProvider {
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
        body.insert("max_completion_tokens".into(), json!(max_output_tokens));
    }
    if let Some(response_format) = options.response_format {
        body.insert(
            "response_format".into(),
            json!(response_format_payload(response_format)),
        );
    }

    Value::Object(body)
}

fn messages(context: &Context) -> Vec<Value> {
    let mut messages = Vec::new();
    if let Some(system) = system_message(context.system.as_deref(), &context.tools) {
        messages.push(json!({
            "role": "system",
            "content": system,
        }));
    }
    messages.extend(context.messages.iter().map(message_input));
    messages
}

fn system_message(system: Option<&str>, tools: &[ToolSpec]) -> Option<String> {
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
        ReasoningLevel::Minimal => "minimal",
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
        Message::ToolResult(message) => json!({
            "role": "tool",
            "tool_call_id": message.tool_call_id,
            "content": tool_result_text(message.content.as_slice()),
        }),
    }
}

fn content_input(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { text } => json!({
            "type": "text",
            "text": text,
        }),
        ContentBlock::Image { mime, data_base64 } => json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{mime};base64,{data_base64}")
            },
        }),
        ContentBlock::Thinking { thinking } => json!({
            "type": "text",
            "text": thinking,
        }),
        ContentBlock::ToolCall {
            id,
            name,
            arguments_json,
        } => json!({
            "type": "tool_call",
            "id": id,
            "function": {
                "name": name,
                "arguments": arguments_json,
            },
        }),
    }
}

fn assistant_content(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Thinking { thinking } => Some(thinking.as_str()),
            ContentBlock::Image { .. } | ContentBlock::ToolCall { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn tool_result_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Thinking { thinking } => Some(thinking.as_str()),
            ContentBlock::Image { .. } | ContentBlock::ToolCall { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn assistant_message_from_response(payload: Value) -> Result<AssistantMessage, ModelError> {
    let choice = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| {
            ModelError::Protocol("chat completions response missing `choices[0]`".into())
        })?;
    let message = choice.get("message").ok_or_else(|| {
        ModelError::Protocol("chat completions response missing assistant message".into())
    })?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ModelError::Protocol("chat completions response missing assistant `content`".into())
        })?;

    let mut blocks = Vec::new();
    if let Some(thinking) = message.get("reasoning_content").and_then(Value::as_str) {
        if !thinking.is_empty() {
            blocks.push(ContentBlock::Thinking {
                thinking: thinking.to_string(),
            });
        }
    }
    if !content.is_empty() {
        blocks.push(ContentBlock::Text {
            text: content.to_string(),
        });
    }

    if blocks.is_empty() {
        return Err(ModelError::Protocol(
            "chat completions response missing assistant content".into(),
        ));
    }

    Ok(AssistantMessage {
        content: blocks,
        usage: usage_from_chat_payload(&payload),
        stop: stop_reason_from_choice(choice),
        error_message: None,
        timestamp_ms: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    })
}

fn usage_from_chat_payload(payload: &Value) -> Usage {
    let usage = payload.get("usage");
    Usage {
        input_tokens: usage
            .and_then(|usage| usage.get("prompt_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        output_tokens: usage
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        total_tokens: usage
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        cost: None,
    }
}

fn stop_reason_from_choice(choice: &Value) -> StopReason {
    match choice.get("finish_reason").and_then(Value::as_str) {
        Some("stop") => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("content_filter") => StopReason::Error,
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
    ModelError::Provider(format!("chat completions api returned {status}: {parsed}"))
}
