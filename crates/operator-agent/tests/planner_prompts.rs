use operator_agent::{
    model::{AssistantMessage, ContentBlock, Message, StopReason, Usage, UserMessage},
    planner::{
        PlannerContext, PlannerPromptBuilder, PlannerVisualInput, PlannerVisualSlot, TargetSummary,
        ToolResultSummary,
    },
    session::{AgentMessage, ModelContextBuffer, VisualObservationSummary},
    tools::AgentToolSpec,
};
use operator_core::{ArtifactId, TargetId};
use serde_json::json;

fn text_block(text: &str) -> ContentBlock {
    ContentBlock::Text {
        text: text.to_string(),
    }
}

fn user_message(text: &str, timestamp_ms: u64) -> Message {
    Message::User(UserMessage {
        content: vec![text_block(text)],
        timestamp_ms,
    })
}

fn assistant_message(text: &str, timestamp_ms: u64) -> Message {
    Message::Assistant(AssistantMessage {
        content: vec![text_block(text)],
        usage: Usage::default(),
        stop: StopReason::Stop,
        error_message: None,
        timestamp_ms,
    })
}

fn planner_context() -> PlannerContext {
    PlannerContext {
        target: TargetSummary {
            id: TargetId("macos".into()),
            platform: "macos".into(),
            capabilities: vec![
                "app_lifecycle".into(),
                "capture".into(),
                "inspect_tree".into(),
            ],
        },
        include_elements: true,
        recent_tool_results: vec![ToolResultSummary {
            turn_index: 1,
            step_index: 1,
            tool_name: "observe".into(),
            is_error: false,
            read_only: true,
            summary: "snapshot snap-1 on frontmost (roots=1, elements=2, screenshot=capture-1.png)"
                .into(),
        }],
        current_observation: Some(VisualObservationSummary {
            snapshot_id: "snap-1".into(),
            surface: "frontmost".into(),
            root_element_count: 1,
            element_count: 2,
            screenshot_artifact: Some(ArtifactId("capture-1.png".into())),
        }),
        current_visual_artifact: Some(ArtifactId("capture-1.png".into())),
        previous_visual_artifact: Some(ArtifactId("capture-prev.png".into())),
        notes: vec!["Observe again before finishing.".into()],
        ui_state_stale: true,
    }
}

fn visual_inputs() -> Vec<PlannerVisualInput> {
    vec![
        PlannerVisualInput {
            slot: PlannerVisualSlot::Previous,
            image: ContentBlock::Image {
                mime: "image/png".into(),
                data_base64: "cHJldmlvdXM=".into(),
            },
        },
        PlannerVisualInput {
            slot: PlannerVisualSlot::Current,
            image: ContentBlock::Image {
                mime: "image/png".into(),
                data_base64: "Y3VycmVudA==".into(),
            },
        },
    ]
}

#[test]
fn planner_prompts_build_json_first_contract_snapshot() {
    let builder = PlannerPromptBuilder::new();
    let tools = vec![
        AgentToolSpec {
            name: "observe".into(),
            description: "Capture a surface and persist the resulting snapshot.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "surface": { "type": "string" }
                }
            }),
            read_only: true,
        },
        AgentToolSpec {
            name: "click".into(),
            description: "Click a locator, coordinate, or target.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "number" },
                    "y": { "type": "number" }
                }
            }),
            read_only: false,
        },
    ];
    let transcript = vec![
        AgentMessage::from(user_message("Open Finder.", 1)),
        AgentMessage::from(assistant_message("I will inspect the desktop first.", 2)),
        AgentMessage::custom(
            "planner.feedback.v1",
            json!({
                "reason": "Need another observe."
            }),
        ),
    ];
    let mut model_context = ModelContextBuffer::new();
    for message in transcript {
        model_context.push(message);
    }

    let context = builder.assemble(
        "Open Finder and confirm the window appears.",
        &planner_context(),
        &tools,
        &model_context,
        &visual_inputs(),
    );

    insta::assert_json_snapshot!(
        "planner_prompts_build_json_first_contract",
        serde_json::to_value(&context).expect("planner context should serialize")
    );
}

#[test]
fn planner_prompts_limit_recent_transcript_before_appending_current_request_snapshot() {
    let builder = PlannerPromptBuilder::new().with_recent_message_limit(2);
    let transcript = vec![
        AgentMessage::from(user_message("earliest", 1)),
        AgentMessage::from(assistant_message("middle", 2)),
        AgentMessage::from(assistant_message("latest assistant", 3)),
        AgentMessage::custom("parser.feedback.v1", json!({ "error": "invalid json" })),
    ];
    let mut model_context = ModelContextBuffer::new();
    for message in transcript {
        model_context.push(message);
    }

    let context = builder.assemble(
        "Retry with valid JSON.",
        &planner_context(),
        &[],
        &model_context,
        &visual_inputs(),
    );
    insta::assert_json_snapshot!(
        "planner_prompts_recent_transcript_window",
        serde_json::to_value(&context.messages).expect("messages should serialize")
    );
}

#[test]
fn planner_prompts_bound_recent_transcript_by_char_budget() {
    let builder = PlannerPromptBuilder::new()
        .with_recent_message_limit(4)
        .with_recent_message_char_limit(80);
    let mut model_context = ModelContextBuffer::new();
    model_context.push(AgentMessage::from(user_message("earliest", 1)));
    model_context.push(AgentMessage::from(assistant_message(&"x".repeat(200), 2)));
    model_context.push(AgentMessage::custom(
        "planner.feedback.v1",
        json!({
            "reason": "Need another observe because the UI changed after clicking Save."
        }),
    ));

    let context = builder.assemble(
        "Retry with valid JSON.",
        &planner_context(),
        &[],
        &model_context,
        &[],
    );

    assert_eq!(
        context.messages.len(),
        2,
        "char budget should keep only the newest compact history entry plus the current request"
    );
    let feedback = serde_json::to_string(&context.messages[0]).expect("message should serialize");
    assert!(feedback.contains("planner.feedback.v1"));
    assert!(
        !feedback.contains(&"x".repeat(50)),
        "long older history should be dropped once the character budget is exhausted"
    );
}

#[test]
fn planner_prompts_forbid_click_based_app_launching_in_system_prompt() {
    let context = PlannerPromptBuilder::new().assemble(
        "Open Notes.",
        &planner_context(),
        &[],
        &ModelContextBuffer::new(),
        &[],
    );

    let system = context
        .system
        .expect("planner prompt should include system text");
    assert!(
        system.contains(
            "use app lifecycle tools such as `launch-app`, `switch-app`, or `relaunch-app`"
        ),
        "system prompt should require app lifecycle tools for app-opening actions: {system}"
    );
    assert!(
        system.contains("Do not use `click` on desktop icons, dock/taskbar items, launcher surfaces, or guessed coordinates to open an app."),
        "system prompt should forbid click-based app launching: {system}"
    );
    assert!(
        system.contains("If an app lifecycle tool fails, do not fall back to guessed coordinate clicks to open that app."),
        "system prompt should forbid guessed click fallback after lifecycle-tool failures: {system}"
    );
}
