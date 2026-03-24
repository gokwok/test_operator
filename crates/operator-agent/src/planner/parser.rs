use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AgentError;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum AgentDecision {
    CallTool {
        name: String,
        arguments: Value,
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thought: Option<String>,
    },
    Finish {
        summary: String,
    },
    Fail {
        reason: String,
    },
}

#[derive(Clone, Debug, Default)]
pub struct DecisionParser;

impl DecisionParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, raw: &str) -> Result<AgentDecision, AgentError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(AgentError::Planner(
                "planner output must not be empty".into(),
            ));
        }

        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => parse_decision(value),
            Err(primary_error) => {
                let recovered = extract_first_json_object(trimmed).ok_or_else(|| {
                    AgentError::Planner(format!(
                        "planner output must be a single JSON object: {primary_error}"
                    ))
                })?;

                let value = serde_json::from_str::<Value>(recovered).map_err(|recovery_error| {
                    AgentError::Planner(format!(
                        "planner output must be a single JSON object: {recovery_error}"
                    ))
                })?;

                parse_decision(value)
            }
        }
    }
}

fn parse_decision(value: Value) -> Result<AgentDecision, AgentError> {
    let object = value
        .as_object()
        .ok_or_else(|| AgentError::Planner("planner output must be a JSON object".into()))?;
    let kind = required_string(object, "decision")?;

    match kind {
        "call_tool" => Ok(AgentDecision::CallTool {
            name: required_string(object, "name")?.to_string(),
            arguments: required_value(object, "arguments")?.clone(),
            summary: required_string(object, "summary")?.to_string(),
            thought: optional_string(object, "thought")?.map(ToOwned::to_owned),
        }),
        "finish" => Ok(AgentDecision::Finish {
            summary: required_string(object, "summary")?.to_string(),
        }),
        "fail" => Ok(AgentDecision::Fail {
            reason: required_string(object, "reason")?.to_string(),
        }),
        other => Err(AgentError::Planner(format!(
            "unsupported planner decision: {other}"
        ))),
    }
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AgentError> {
    required_value(object, key)?
        .as_str()
        .ok_or_else(|| AgentError::Planner(format!("planner field `{key}` must be a string")))
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, AgentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_str().map(Some).ok_or_else(|| {
            AgentError::Planner(format!(
                "planner field `{key}` must be a string when present"
            ))
        }),
    }
}

fn required_value<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Value, AgentError> {
    object
        .get(key)
        .ok_or_else(|| AgentError::Planner(format!("planner output is missing `{key}`")))
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
