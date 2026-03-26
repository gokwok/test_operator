use std::sync::Arc;

use serde_json::json;

use crate::{
    model::{ContentBlock, Context, Message, ToolSpec, UserMessage},
    session::{AgentMessage, ModelContextBuffer},
    tools::AgentToolSpec,
};

use super::{PlannerContext, PlannerVisualSlot};

const DEFAULT_RECENT_MESSAGES: usize = 8;
const PLANNER_SYSTEM_PROMPT: &str = concat!(
    "You are the Operator planner.\n",
    "Choose exactly one next decision for the current desktop automation task.\n",
    "Use only the provided tools and the transcript/context you are given.\n",
    "The runner may already provide automatic screenshot-only observe results on the hot path.\n",
    "The planner context carries current/previous visual artifact references from the in-memory loop state, not full snapshot bodies.\n",
    "Do not finish while `ui_state_stale` is true.\n",
    "Use `observe` as a cold-path tool when you need to verify UI content or state; request `include_elements=true` because screenshot-only or empty observations do not count as verification.\n",
    "Do not invent tool results, hidden UI state, or unsupported tool arguments.\n",
    "Return exactly one JSON object and no surrounding prose.\n",
    "Valid decision shapes:\n",
    "{\"decision\":\"call_tool\",\"name\":\"<tool-name>\",\"arguments\":{},\"summary\":\"<brief next-step summary>\",\"thought\":\"<optional reasoning>\"}\n",
    "{\"decision\":\"finish\",\"summary\":\"<why the task is complete>\"}\n",
    "{\"decision\":\"fail\",\"reason\":\"<why the task cannot continue>\"}",
);

#[derive(Clone, Debug, PartialEq)]
pub struct PlannerVisualInput {
    pub slot: PlannerVisualSlot,
    pub image: ContentBlock,
}

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
        model_context: &ModelContextBuffer,
        visual_inputs: &[PlannerVisualInput],
    ) -> Context {
        let mut messages = self.recent_model_context_messages(model_context);
        messages.push(Message::User(UserMessage {
            content: current_request_content(task, planner_context, visual_inputs),
            timestamp_ms: 0,
        }));

        Context {
            system: Some(PLANNER_SYSTEM_PROMPT.to_string()),
            messages,
            tools: tools.iter().map(tool_spec).collect(),
        }
    }

    fn recent_model_context_messages(&self, model_context: &ModelContextBuffer) -> Vec<Message> {
        let messages = model_context.messages();
        let start = messages.len().saturating_sub(self.recent_message_limit);
        messages[start..].iter().map(transcript_message).collect()
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
        "current_observation": planner_context.current_observation,
        "notes": planner_context.notes,
        "ui_state_stale": planner_context.ui_state_stale,
    })
}

fn current_request_content(
    task: &str,
    planner_context: &PlannerContext,
    visual_inputs: &[PlannerVisualInput],
) -> Vec<ContentBlock> {
    let mut content = vec![ContentBlock::Text {
        text: serialize_pretty_json(current_request(task, planner_context)),
    }];
    for visual in visual_inputs {
        content.push(ContentBlock::Text {
            text: visual_label(visual.slot).to_string(),
        });
        content.push(visual.image.clone());
    }
    content
}

fn serialize_pretty_json(value: serde_json::Value) -> String {
    serde_json::to_string_pretty(&value).expect("planner prompt payloads should serialize")
}

fn visual_label(slot: PlannerVisualSlot) -> &'static str {
    match slot {
        PlannerVisualSlot::Previous => "Previous screenshot (older context).",
        PlannerVisualSlot::Current => "Current screenshot (latest UI state).",
    }
}
