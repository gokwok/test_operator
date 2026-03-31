use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    model::{ContentBlock, Context, Message, ModelRequest, ResolvedModel, UserMessage},
    session::{AgentSessionState, LoopHistoryItem},
    AgentError,
};

const FINISH_GATE_SYSTEM_PROMPT: &str = concat!(
    "You are the Operator finish gate.\n",
    "Decide whether the desktop automation task is actually complete.\n",
    "Use only the original task, the finish summary, recent history, notes, and current/previous visual references.\n",
    "Do not rely on a full transcript or a full tool trace.\n",
    "Return exactly one JSON object and no surrounding prose.\n",
    "Valid verdict shapes:\n",
    "{\"verdict\":\"ok\",\"reason\":\"<why the task is complete>\"}\n",
    "{\"verdict\":\"not_ok\",\"reason\":\"<what is still missing or unverified>\"}",
);
const STALE_UI_REASON: &str =
    "The task is not verified yet because there is no fresh usable observe result after the last UI change.";
const MISSING_OBSERVATION_REASON: &str =
    "The task is not verified yet because there is no usable observe result in the current loop state.";
const DETERMINISTIC_OK_REASON: &str =
    "The task has a fresh usable observe result and no recent side-effect actions require extra finish reflection.";
const DEFAULT_RECENT_HISTORY_LIMIT: usize = 6;
const DEFAULT_RECENT_SIDE_EFFECT_LIMIT: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum FinishGateVerdict {
    Ok { reason: String },
    NotOk { reason: String },
}

#[derive(Clone, Debug)]
pub struct FinishGate {
    recent_history_limit: usize,
    recent_side_effect_limit: usize,
}

impl FinishGate {
    pub fn new() -> Self {
        Self {
            recent_history_limit: DEFAULT_RECENT_HISTORY_LIMIT,
            recent_side_effect_limit: DEFAULT_RECENT_SIDE_EFFECT_LIMIT,
        }
    }

    pub async fn evaluate(
        &self,
        model: &ResolvedModel,
        state: &AgentSessionState,
        finish_summary: &str,
    ) -> Result<FinishGateVerdict, AgentError> {
        match self.deterministic_verdict(state) {
            DeterministicVerdict::Ok { reason } => {
                return Ok(FinishGateVerdict::Ok {
                    reason: reason.to_string(),
                });
            }
            DeterministicVerdict::NotOk { reason } => {
                return Ok(FinishGateVerdict::NotOk {
                    reason: reason.to_string(),
                });
            }
            DeterministicVerdict::Reflect => {}
        }

        let request = ModelRequest {
            config: model.config.clone(),
            context: reflection_context(self.recent_history(state), state, finish_summary),
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
                AgentError::Planner(format!("finish gate model call failed: {error}"))
            })?;
        let raw = assistant_text(&message.content)?;

        parse_verdict(&raw)
    }

    pub fn record_feedback(&self, state: &mut AgentSessionState, verdict: &FinishGateVerdict) {
        if let FinishGateVerdict::NotOk { reason } = verdict {
            state.add_note(reason.clone());
        }
    }

    fn deterministic_verdict(&self, state: &AgentSessionState) -> DeterministicVerdict {
        if state.ui_state_stale {
            return DeterministicVerdict::NotOk {
                reason: STALE_UI_REASON,
            };
        }

        if !has_usable_observation(state) {
            return DeterministicVerdict::NotOk {
                reason: MISSING_OBSERVATION_REASON,
            };
        }

        if needs_reflection(state, self.recent_side_effect_limit) {
            DeterministicVerdict::Reflect
        } else {
            DeterministicVerdict::Ok {
                reason: DETERMINISTIC_OK_REASON,
            }
        }
    }

    fn recent_history(&self, state: &AgentSessionState) -> Vec<LoopHistoryItem> {
        let start = state
            .history
            .len()
            .saturating_sub(self.recent_history_limit);
        state.history[start..].to_vec()
    }
}

impl Default for FinishGate {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeterministicVerdict {
    Ok { reason: &'static str },
    NotOk { reason: &'static str },
    Reflect,
}

fn needs_reflection(state: &AgentSessionState, recent_side_effect_limit: usize) -> bool {
    !state.notes.is_empty()
        || state
            .tool_trace
            .iter()
            .rev()
            .filter(|entry| !entry.result.is_error)
            .take(recent_side_effect_limit)
            .any(|entry| !entry.result.read_only)
}

fn has_usable_observation(state: &AgentSessionState) -> bool {
    state
        .current_observation()
        .is_some_and(|summary| summary.is_usable(state.include_elements()))
}

fn reflection_context(
    recent_history: Vec<LoopHistoryItem>,
    state: &AgentSessionState,
    finish_summary: &str,
) -> Context {
    Context {
        system: Some(FINISH_GATE_SYSTEM_PROMPT.to_string()),
        messages: vec![Message::User(UserMessage {
            content: vec![ContentBlock::Text {
                text: serialize_pretty_json(json!({
                    "task": state.task,
                    "finish_summary": finish_summary,
                    "notes": state.notes,
                    "recent_history": recent_history,
                    "current_observation": state.current_observation(),
                    "current_visual_artifact": state.current_visual(),
                    "previous_visual_artifact": state.previous_visual(),
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
            "finish gate response must contain at least one text block".into(),
        ));
    }

    Ok(trimmed.to_owned())
}

fn parse_verdict(raw: &str) -> Result<FinishGateVerdict, AgentError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AgentError::Planner(
            "finish gate response must not be empty".into(),
        ));
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => verdict_from_value(value),
        Err(primary_error) => {
            let recovered = extract_first_json_object(trimmed).ok_or_else(|| {
                AgentError::Planner(format!(
                    "finish gate response must be a single JSON object: {primary_error}"
                ))
            })?;

            let value = serde_json::from_str::<Value>(recovered).map_err(|recovery_error| {
                AgentError::Planner(format!(
                    "finish gate response must be a single JSON object: {recovery_error}"
                ))
            })?;

            verdict_from_value(value)
        }
    }
}

fn verdict_from_value(value: Value) -> Result<FinishGateVerdict, AgentError> {
    let object = value
        .as_object()
        .ok_or_else(|| AgentError::Planner("finish gate response must be a JSON object".into()))?;
    let verdict = required_string(object, "verdict")?;
    let reason = required_string(object, "reason")?.to_string();

    match verdict {
        "ok" => Ok(FinishGateVerdict::Ok { reason }),
        "not_ok" => Ok(FinishGateVerdict::NotOk { reason }),
        other => Err(AgentError::Planner(format!(
            "finish gate verdict must be `ok` or `not_ok`, got `{other}`"
        ))),
    }
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AgentError> {
    object
        .get(key)
        .ok_or_else(|| AgentError::Planner(format!("finish gate response is missing `{key}`")))?
        .as_str()
        .ok_or_else(|| AgentError::Planner(format!("finish gate field `{key}` must be a string")))
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
    serde_json::to_string_pretty(&value).expect("finish gate payloads should serialize")
}
