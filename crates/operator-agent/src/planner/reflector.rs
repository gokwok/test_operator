use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    model::{ContentBlock, Context, Message, ModelRequest, ResolvedModel, UserMessage},
    session::AgentSessionState,
    AgentError,
};

const REFLECTOR_SYSTEM_PROMPT: &str = concat!(
    "You are the Operator task reflector.\n",
    "Decide whether the desktop automation task is actually complete.\n",
    "Use the original task, finish summary, transcript, notes, and tool trace.\n",
    "Return exactly one JSON object and no surrounding prose.\n",
    "Valid verdict shapes:\n",
    "{\"verdict\":\"ok\",\"reason\":\"<why the task is complete>\"}\n",
    "{\"verdict\":\"not_ok\",\"reason\":\"<what is still missing or unverified>\"}",
);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum TaskReflection {
    Ok { reason: String },
    NotOk { reason: String },
}

#[derive(Clone, Debug, Default)]
pub struct TaskReflector;

impl TaskReflector {
    pub fn new() -> Self {
        Self
    }

    pub async fn reflect(
        &self,
        model: &ResolvedModel,
        state: &AgentSessionState,
        finish_summary: &str,
    ) -> Result<TaskReflection, AgentError> {
        let request = ModelRequest {
            config: model.config.clone(),
            context: reflection_context(state, finish_summary),
            options: model.config.default_options.clone(),
            stream: false,
            timeout: model.config.default_timeout_ms.map(Duration::from_millis),
            request_id: None,
            max_retry_delay_ms: None,
        };

        let message = model
            .provider
            .stream(request)
            .result()
            .await
            .map_err(|error| {
                AgentError::Planner(format!("reflector model call failed: {error}"))
            })?;
        let raw = assistant_text(&message.content)?;

        parse_reflection(&raw)
    }

    pub fn record_feedback(&self, state: &mut AgentSessionState, reflection: &TaskReflection) {
        if let TaskReflection::NotOk { reason } = reflection {
            state.add_note(reason.clone());
        }
    }
}

fn reflection_context(state: &AgentSessionState, finish_summary: &str) -> Context {
    Context {
        system: Some(REFLECTOR_SYSTEM_PROMPT.to_string()),
        messages: vec![Message::User(UserMessage {
            content: vec![ContentBlock::Text {
                text: serialize_pretty_json(json!({
                    "task": state.task,
                    "finish_summary": finish_summary,
                    "notes": state.notes,
                    "transcript": state.messages,
                    "tool_trace": state.tool_trace,
                })),
            }],
            timestamp_ms: 0,
        })],
        tools: Vec::new(),
    }
}

fn assistant_text(blocks: &[ContentBlock]) -> Result<String, AgentError> {
    let text = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AgentError::Planner(
            "reflector response must contain at least one text block".into(),
        ));
    }

    Ok(trimmed.to_owned())
}

fn parse_reflection(raw: &str) -> Result<TaskReflection, AgentError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AgentError::Planner(
            "reflector response must not be empty".into(),
        ));
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => reflection_from_value(value),
        Err(primary_error) => {
            let recovered = extract_first_json_object(trimmed).ok_or_else(|| {
                AgentError::Planner(format!(
                    "reflector response must be a single JSON object: {primary_error}"
                ))
            })?;

            let value = serde_json::from_str::<Value>(recovered).map_err(|recovery_error| {
                AgentError::Planner(format!(
                    "reflector response must be a single JSON object: {recovery_error}"
                ))
            })?;

            reflection_from_value(value)
        }
    }
}

fn reflection_from_value(value: Value) -> Result<TaskReflection, AgentError> {
    let object = value
        .as_object()
        .ok_or_else(|| AgentError::Planner("reflector response must be a JSON object".into()))?;
    let verdict = required_string(object, "verdict")?;
    let reason = required_string(object, "reason")?.to_string();

    match verdict {
        "ok" => Ok(TaskReflection::Ok { reason }),
        "not_ok" => Ok(TaskReflection::NotOk { reason }),
        other => Err(AgentError::Planner(format!(
            "reflector verdict must be `ok` or `not_ok`, got `{other}`"
        ))),
    }
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AgentError> {
    object
        .get(key)
        .ok_or_else(|| AgentError::Planner(format!("reflector response is missing `{key}`")))?
        .as_str()
        .ok_or_else(|| AgentError::Planner(format!("reflector field `{key}` must be a string")))
}

fn extract_first_json_object(raw: &str) -> Option<&str> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in raw.char_indices() {
        if let Some(start_index) = start {
            if in_string {
                if escaped {
                    escaped = false;
                    continue;
                }

                match ch {
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return raw.get(start_index..=index);
                    }
                }
                _ => {}
            }
        } else if ch == '{' {
            start = Some(index);
            depth = 1;
        }
    }

    None
}

fn serialize_pretty_json(value: Value) -> String {
    serde_json::to_string_pretty(&value).expect("reflector payloads should serialize")
}
