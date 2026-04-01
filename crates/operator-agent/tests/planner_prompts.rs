use operator_agent::{
    model::{
        AssistantMessage, CallOptions, ContentBlock, CoordinatePolicy, Message, ModelConfig,
        ProviderKind, StopReason, Usage, UserMessage,
    },
    planner::{
        PlannerContext, PlannerPromptBuilder, PlannerVisualInput, PlannerVisualSlot, TargetSummary,
        ToolResultSummary,
    },
    session::{
        AgentMessage, BootstrapAppCatalog, BootstrapAppCatalogEntry, BootstrapAppContext,
        ElementDigest, ElementDigestEntry, ModelContextBuffer, VisualObservationSummary,
    },
    tools::AgentToolSpec,
};
use operator_core::{ArtifactId, ImageSizePx, Rect, TargetId};
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

fn current_request_text(messages: &[Message]) -> String {
    let Message::User(UserMessage { content, .. }) = messages
        .last()
        .expect("planner request should append a user message")
    else {
        panic!("last planner message should be a user request");
    };
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
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
            image_size_px: Some(ImageSizePx {
                width: 1260,
                height: 2720,
            }),
            element_tree_reliability: None,
            element_tree_note: None,
            element_digest: None,
        }),
        current_visual_artifact: Some(ArtifactId("capture-1.png".into())),
        previous_visual_artifact: Some(ArtifactId("capture-prev.png".into())),
        notes: vec!["Observe again before finishing.".into()],
        app_bootstrap: None,
        ui_state_stale: true,
    }
}

fn digest_observation() -> VisualObservationSummary {
    VisualObservationSummary {
        snapshot_id: "snap-digest".into(),
        surface: "frontmost".into(),
        root_element_count: 1,
        element_count: 2,
        screenshot_artifact: Some(ArtifactId("capture-digest.png".into())),
        image_size_px: None,
        element_tree_reliability: None,
        element_tree_note: None,
        element_digest: Some(ElementDigest {
            entries: vec![
                ElementDigestEntry {
                    element_id: "el-button".into(),
                    role: "button".into(),
                    label: Some("保存".into()),
                    value: None,
                    enabled: Some(true),
                    bounds: Some(Rect {
                        x: 12.0,
                        y: 24.0,
                        width: 88.0,
                        height: 36.0,
                    }),
                    depth: 0,
                },
                ElementDigestEntry {
                    element_id: "el-text".into(),
                    role: "text".into(),
                    label: Some("状态：已保存".into()),
                    value: None,
                    enabled: None,
                    bounds: Some(Rect {
                        x: 18.0,
                        y: 70.0,
                        width: 120.0,
                        height: 20.0,
                    }),
                    depth: 1,
                },
            ],
            truncated_count: 2,
        }),
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

fn openai_model_config() -> ModelConfig {
    ModelConfig {
        provider: ProviderKind::OpenAi,
        id: "gpt-5.4".into(),
        coordinate_policy: CoordinatePolicy::SurfaceImagePixels,
        default_options: CallOptions::default(),
        default_timeout_ms: Some(30_000),
    }
}

fn compatible_model_config() -> ModelConfig {
    ModelConfig {
        provider: ProviderKind::OpenAiCompatible,
        id: "doubao-seed-2-0-lite-260215".into(),
        coordinate_policy: CoordinatePolicy::SurfaceNormalized1000,
        default_options: CallOptions::default(),
        default_timeout_ms: Some(30_000),
    }
}

#[test]
fn planner_prompts_render_unreliable_element_tree_warning() {
    let mut context = planner_context();
    context.current_observation = Some(VisualObservationSummary {
        snapshot_id: "snap-warning".into(),
        surface: "frontmost".into(),
        root_element_count: 1,
        element_count: 4,
        screenshot_artifact: Some(ArtifactId("capture-warning.png".into())),
        image_size_px: None,
        element_tree_reliability: Some(operator_core::ElementTreeReliability::Unreliable),
        element_tree_note: Some(
            "Harmony element tree is too sparse for reliable no-vision interaction on this screen; prefer pure-vision (screenshot-only) mode.".into(),
        ),
        element_digest: None,
    });
    context.current_visual_artifact = Some(ArtifactId("capture-warning.png".into()));

    let assembled = PlannerPromptBuilder::new().assemble(
        "Inspect the current UI.",
        &compatible_model_config(),
        &context,
        &[],
        &ModelContextBuffer::new(),
        &[],
    );
    let request = current_request_text(&assembled.messages);

    assert!(request.contains("element tree reliability: unreliable"));
    assert!(request.contains("prefer pure-vision (screenshot-only) mode"));
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
        &compatible_model_config(),
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
        &compatible_model_config(),
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
        &compatible_model_config(),
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
        &compatible_model_config(),
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

#[test]
fn planner_prompts_include_bootstrap_app_catalog_and_prelaunched_app_in_system_prompt() {
    let mut context = planner_context();
    context.app_bootstrap = Some(BootstrapAppContext {
        prelaunched_app: Some("备忘录".into()),
        installed_catalog: Some(BootstrapAppCatalog {
            total_count: 2,
            entries: vec![
                BootstrapAppCatalogEntry {
                    name: "备忘录".into(),
                    bundle_id: Some("com.huawei.hmos.notepad".into()),
                    is_running: true,
                },
                BootstrapAppCatalogEntry {
                    name: "计算器".into(),
                    bundle_id: Some("com.huawei.hmos.calculator".into()),
                    is_running: false,
                },
            ],
            truncated_count: 0,
        }),
    });

    let assembled = PlannerPromptBuilder::new().assemble(
        "Open Notes.",
        &compatible_model_config(),
        &context,
        &[],
        &ModelContextBuffer::new(),
        &[],
    );
    let system = assembled
        .system
        .expect("planner prompt should include system text");

    assert!(system
        .contains("The CLI already prelaunched this app before the first planner turn: 备忘录"));
    assert!(system.contains("Installed app catalog bootstrap (`app list --all`):"));
    assert!(system.contains("备忘录 [bundle=com.huawei.hmos.notepad] [running]"));
    assert!(system.contains("计算器 [bundle=com.huawei.hmos.calculator]"));
}

#[test]
fn planner_prompts_include_openai_grounding_contract_and_image_size_only_for_openai() {
    let assembled = PlannerPromptBuilder::new().assemble(
        "Tap the yellow plus button.",
        &openai_model_config(),
        &planner_context(),
        &[],
        &ModelContextBuffer::new(),
        &visual_inputs(),
    );

    let system = assembled
        .system
        .expect("planner prompt should include system text");
    assert!(system.contains("OpenAI screenshot grounding contract:"));
    assert!(system.contains("Current screenshot pixel size: 1260 x 2720."));
    assert!(system
        .contains("Never use normalized coordinates, percentages, or screen-global coordinates."));
    assert!(system.contains("first internally estimate a tight bounding box"));
    assert!(system.contains("prefer the bbox center or slightly above center"));

    let request = serde_json::to_string(&assembled.messages)
        .expect("rendered planner request should serialize");
    assert!(request.contains("screenshot image_size_px: 1260 x 2720"));
    assert!(request.contains(
        "screenshot coordinate space: original image pixels with origin=(0,0) at the top-left"
    ));
}

#[test]
fn planner_prompts_do_not_include_openai_grounding_for_compatible_provider() {
    let assembled = PlannerPromptBuilder::new().assemble(
        "Tap the yellow plus button.",
        &compatible_model_config(),
        &planner_context(),
        &[],
        &ModelContextBuffer::new(),
        &visual_inputs(),
    );

    let system = assembled
        .system
        .expect("planner prompt should include system text");
    assert!(
        !system.contains("OpenAI screenshot grounding contract:"),
        "non-OpenAI providers should not receive the OpenAI grounding contract: {system}"
    );
    let request = serde_json::to_string(&assembled.messages)
        .expect("rendered planner request should serialize");
    assert!(
        !request.contains("screenshot coordinate space: original image pixels"),
        "non-OpenAI providers should not receive the coordinate-space hint: {request}"
    );
    assert!(
        !request.contains("screenshot image_size_px: 1260 x 2720"),
        "non-OpenAI providers should not receive OpenAI-only image-size hints: {request}"
    );
}

#[test]
fn planner_prompts_render_bounded_element_digest_lines() {
    let mut context = planner_context();
    context.current_observation = Some(digest_observation());
    context.current_visual_artifact = Some(ArtifactId("capture-digest.png".into()));

    let assembled = PlannerPromptBuilder::new().assemble(
        "Inspect the current UI.",
        &compatible_model_config(),
        &context,
        &[],
        &ModelContextBuffer::new(),
        &[],
    );
    let request = current_request_text(&assembled.messages);

    assert!(request
        .contains("element digest (SnapshotElement ids; native bounds use device coordinates):"));
    assert!(request.contains("[el-button] button label=\"保存\" enabled=true bounds=(12,24,88,36)"));
    assert!(request.contains("[el-text] text label=\"状态：已保存\""));
    assert!(request.contains("... 2 more element digest entries omitted"));
}
