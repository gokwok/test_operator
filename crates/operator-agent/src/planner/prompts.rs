use std::sync::Arc;

use crate::{
    model::{Context, Message, ToolSpec, UserMessage},
    session::ModelContextBuffer,
    tools::AgentToolSpec,
};

use super::{PlannerContext, PlannerRenderer, PlannerVisualInput};

const DEFAULT_RECENT_MESSAGES: usize = 8;
const DEFAULT_RECENT_MESSAGE_CHARS: usize = 1600;
const PLANNER_SYSTEM_PROMPT: &str = concat!(
    "You are the Operator planner.\n",
    "Choose exactly one next decision for the current desktop automation task.\n",
    "Use only the provided tools and the transcript/context you are given.\n",
    "The runner may already provide automatic screenshot-only observe results on the hot path.\n",
    "The planner context carries current/previous visual artifact references from the in-memory loop state, not full snapshot bodies.\n",
    "Do not finish while `ui_state_stale` is true.\n",
    "Use `observe` as a cold-path tool when you need to verify UI content or state; follow the planner context's observe verification mode when deciding whether to request `include_elements=true`.\n",
    "Enum values and field names are case-sensitive; copy them exactly from the provided tool summaries.\n",
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
    recent_message_char_limit: usize,
    renderer: PlannerRenderer,
}

impl PlannerPromptBuilder {
    pub fn new() -> Self {
        Self {
            recent_message_limit: DEFAULT_RECENT_MESSAGES,
            recent_message_char_limit: DEFAULT_RECENT_MESSAGE_CHARS,
            renderer: PlannerRenderer::new(),
        }
    }

    pub fn with_recent_message_limit(mut self, recent_message_limit: usize) -> Self {
        self.recent_message_limit = recent_message_limit;
        self
    }

    pub fn with_recent_message_char_limit(mut self, recent_message_char_limit: usize) -> Self {
        self.recent_message_char_limit = recent_message_char_limit;
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
            content: self
                .renderer
                .render_request(task, planner_context, visual_inputs),
            timestamp_ms: 0,
        }));

        Context {
            system: Some(PLANNER_SYSTEM_PROMPT.to_string()),
            messages,
            tools: tools.iter().map(tool_spec).collect(),
        }
    }

    fn recent_model_context_messages(&self, model_context: &ModelContextBuffer) -> Vec<Message> {
        model_context.planner_messages(self.recent_message_limit, self.recent_message_char_limit)
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
        input_schema: serde_json::to_value(spec.planner_summary())
            .expect("planner tool summaries should serialize"),
    }
}
