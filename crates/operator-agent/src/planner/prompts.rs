use std::sync::Arc;

use serde_json::json;

use crate::{
    model::{Context, Message, ToolSpec, UserMessage},
    session::AgentMessage,
    tools::AgentToolSpec,
};

use super::PlannerContext;

const DEFAULT_RECENT_MESSAGES: usize = 8;
const PLANNER_SYSTEM_PROMPT: &str = concat!(
    "You are the Operator planner.\n",
    "Choose exactly one next decision for the current desktop automation task.\n",
    "Use only the provided tools and the transcript/context you are given.\n",
    "The runner may already provide automatic screenshot-only observe results on the hot path.\n",
    "Do not finish while `ui_state_stale` is true.\n",
    "Use `observe` as a cold-path tool when you need to verify UI content or state; request `include_elements=true` because screenshot-only or empty observations do not count as verification.\n",
    "Do not invent tool results, hidden UI state, or unsupported tool arguments.\n",
    "Return exactly one JSON object and no surrounding prose.\n",
    "Valid decision shapes:\n",
    "{\"decision\":\"call_tool\",\"name\":\"<tool-name>\",\"arguments\":{},\"summary\":\"<brief next-step summary>\",\"thought\":\"<optional reasoning>\"}\n",
    "{\"decision\":\"finish\",\"summary\":\"<why the task is complete>\"}\n",
    "{\"decision\":\"fail\",\"reason\":\"<why the task cannot continue>\"}",
);

#[derive(Clone, Debug)]
pub struct PlannerPromptBuilder {
    recent_message_limit: usize,
}

impl PlannerPromptBuilder {
    pub fn new() -> Self {
        Self {
            recent_message_limit: DEFAULT_RECENT_MESSAGES,
        }
    }

    pub fn with_recent_message_limit(mut self, recent_message_limit: usize) -> Self {
        self.recent_message_limit = recent_message_limit;
        self
    }

    pub fn assemble(
        &self,
        task: &str,
        planner_context: &PlannerContext,
        tools: &[AgentToolSpec],
        transcript: &[AgentMessage],
    ) -> Context {
        let mut messages = self.recent_transcript_messages(transcript);
        messages.push(Message::User(UserMessage {
            content: vec![crate::model::ContentBlock::Text {
                text: serialize_pretty_json(current_request(task, planner_context)),
            }],
            timestamp_ms: 0,
        }));

        Context {
            system: Some(PLANNER_SYSTEM_PROMPT.to_string()),
            messages,
            tools: tools.iter().map(tool_spec).collect(),
        }
    }

    fn recent_transcript_messages(&self, transcript: &[AgentMessage]) -> Vec<Message> {
        let start = transcript.len().saturating_sub(self.recent_message_limit);
        transcript[start..].iter().map(transcript_message).collect()
    }
}

impl Default for PlannerPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn tool_spec(spec: &AgentToolSpec) -> ToolSpec {
    ToolSpec {
        name: Arc::<str>::from(spec.name.as_str()),
        description: Arc::<str>::from(spec.description.as_str()),
        input_schema: spec.input_schema.clone(),
    }
}

fn transcript_message(message: &AgentMessage) -> Message {
    match message {
        AgentMessage::Base { message } => message.clone(),
        AgentMessage::Custom { kind, payload } => Message::User(UserMessage {
            content: vec![crate::model::ContentBlock::Text {
                text: serialize_pretty_json(json!({
                    "kind": kind,
                    "payload": payload,
                })),
            }],
            timestamp_ms: 0,
        }),
    }
}

fn current_request(task: &str, planner_context: &PlannerContext) -> serde_json::Value {
    json!({
        "task": task,
        "target": planner_context.target,
        "recent_tool_results": planner_context.recent_tool_results,
        "latest_snapshot": planner_context.latest_snapshot,
        "previous_snapshot_visual": planner_context.previous_snapshot_visual,
        "notes": planner_context.notes,
        "ui_state_stale": planner_context.ui_state_stale,
    })
}

fn serialize_pretty_json(value: serde_json::Value) -> String {
    serde_json::to_string_pretty(&value).expect("planner prompt payloads should serialize")
}
