use std::sync::Arc;

use crate::{
    model::{Context, CoordinatePolicy, Message, ModelConfig, ProviderKind, ToolSpec, UserMessage},
    session::ModelContextBuffer,
    tools::AgentToolSpec,
};

use super::{
    context::PlannerVisualSlot, renderer::PlannerRenderHints, PlannerContext, PlannerRenderer,
    PlannerVisualInput,
};

const DEFAULT_RECENT_MESSAGES: usize = 8;
const DEFAULT_RECENT_MESSAGE_CHARS: usize = 1600;
const PLANNER_SYSTEM_PROMPT: &str = concat!(
    "You are the Operator planner.\n",
    "Choose exactly one next decision for the current desktop automation task.\n",
    "Use only the provided tools and the transcript/context you are given.\n",
    "The runner may already provide automatic screenshot-only observe results on the hot path.\n",
    "The planner context carries current/previous visual artifact references from the in-memory loop state, not full snapshot bodies.\n",
    "Do not finish while `ui_state_stale` is true.\n",
    "Use `observe` as a cold-path tool when you need to verify UI content or state; follow the planner context's observe verification mode when deciding whether to request `elements=true`.\n",
    "When the next step is to open, switch to, relaunch, or foreground an app, use app lifecycle tools such as `launch-app`, `switch-app`, or `relaunch-app`.\n",
    "Do not use `click` on desktop icons, dock/taskbar items, launcher surfaces, or guessed coordinates to open an app.\n",
    "If an app lifecycle tool fails, do not fall back to guessed coordinate clicks to open that app.\n",
    "Enum values and field names are case-sensitive; copy them exactly from the provided tool summaries.\n",
    "Do not invent tool results, hidden UI state, or unsupported tool arguments.\n",
    "Return exactly one JSON object and no surrounding prose.\n",
    "Valid decision shapes:\n",
    "{\"decision\":\"call_tool\",\"name\":\"<tool-name>\",\"arguments\":{},\"summary\":\"<brief next-step summary>\",\"thought\":\"<optional reasoning>\"}\n",
    "{\"decision\":\"finish\",\"summary\":\"<why the task is complete>\"}\n",
    "{\"decision\":\"fail\",\"reason\":\"<why the task cannot continue>\"}",
);

fn planner_system_prompt(
    planner_context: &PlannerContext,
    openai_screenshot_grounding: bool,
) -> String {
    let mut system = PLANNER_SYSTEM_PROMPT.to_string();

    if let Some(app_bootstrap) = planner_context.app_bootstrap.as_ref() {
        if let Some(app) = app_bootstrap.prelaunched_app.as_deref() {
            system.push_str("\nBootstrap app hint:");
            system.push_str(
                "\n- The CLI already prelaunched this app before the first planner turn: ",
            );
            system.push_str(app);
            system.push_str("\n- Prefer interacting with that app instead of reopening it unless the task requires a relaunch.");
        }

        if let Some(catalog) = app_bootstrap.installed_catalog.as_ref() {
            system.push_str("\nInstalled app catalog bootstrap (`app list --all`):");
            system.push_str(&format!("\n- total apps: {}", catalog.total_count));
            if catalog.entries.is_empty() {
                system.push_str("\n- entries: none");
            } else {
                system.push_str("\n- entries:");
                for entry in &catalog.entries {
                    system.push_str("\n  - ");
                    system.push_str(&entry.name);
                    if let Some(bundle_id) = entry.bundle_id.as_deref() {
                        system.push_str(" [bundle=");
                        system.push_str(bundle_id);
                        system.push(']');
                    }
                    if entry.is_running {
                        system.push_str(" [running]");
                    }
                }
            }
            if catalog.truncated_count > 0 {
                system.push_str(&format!(
                    "\n- truncated: {} additional app entries omitted from the bootstrap catalog",
                    catalog.truncated_count
                ));
            }
        }
    }

    if openai_screenshot_grounding {
        system.push_str("\nOpenAI screenshot grounding contract:");
        if let Some(size) = planner_context
            .current_observation
            .as_ref()
            .and_then(|observation| observation.image_size_px)
        {
            system.push_str(&format!(
                "\n- Current screenshot pixel size: {} x {}.",
                size.width, size.height
            ));
        }
        system.push_str(
            "\n- When using screenshot-based coordinate locators, coordinates must refer to the original pixel grid of the current screenshot image.",
        );
        system.push_str("\n- Origin is the top-left corner of the current screenshot: (0,0).");
        system.push_str("\n- x increases to the right; y increases downward.");
        system.push_str(
            "\n- Never use normalized coordinates, percentages, or screen-global coordinates.",
        );
        system.push_str(
            "\n- Before emitting a screenshot-coordinate action, first internally estimate a tight bounding box for the target in the current screenshot.",
        );
        system.push_str("\n- Then choose the action point from that bbox.");
        system.push_str(
            "\n- For circular floating buttons, prefer the bbox center or slightly above center.",
        );
    }

    if !planner_context
        .current_observation
        .as_ref()
        .is_some_and(|observation| observation.has_elements())
    {
        system.push_str("\nSelector locator availability:");
        system.push_str(
            "\n- Element-ID and text locators are unavailable until the current observe result includes elements.",
        );
        system.push_str(
            "\n- If you need element-based actions, call `observe` with `elements=true` first and wait for that observation to succeed.",
        );
    }

    system
}

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
        model_config: &ModelConfig,
        planner_context: &PlannerContext,
        tools: &[AgentToolSpec],
        model_context: &ModelContextBuffer,
        visual_inputs: &[PlannerVisualInput],
    ) -> Context {
        let openai_screenshot_grounding =
            enable_openai_screenshot_grounding(model_config, planner_context, visual_inputs);
        let mut messages = self.recent_model_context_messages(model_context);
        messages.push(Message::User(UserMessage {
            content: self.renderer.render_request(
                task,
                planner_context,
                visual_inputs,
                PlannerRenderHints {
                    openai_screenshot_coordinate_contract: openai_screenshot_grounding,
                },
            ),
            timestamp_ms: 0,
        }));

        Context {
            system: Some(planner_system_prompt(
                planner_context,
                openai_screenshot_grounding,
            )),
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

fn enable_openai_screenshot_grounding(
    model_config: &ModelConfig,
    planner_context: &PlannerContext,
    visual_inputs: &[PlannerVisualInput],
) -> bool {
    model_config.provider == ProviderKind::OpenAi
        && model_config.coordinate_policy == CoordinatePolicy::SurfaceImagePixels
        && planner_context
            .current_observation
            .as_ref()
            .and_then(|observation| observation.screenshot_artifact.as_ref())
            .is_some()
        && visual_inputs
            .iter()
            .any(|visual| visual.slot == PlannerVisualSlot::Current)
}
