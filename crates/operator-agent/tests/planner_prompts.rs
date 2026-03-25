use operator_agent::{
    model::{AssistantMessage, ContentBlock, Message, StopReason, Usage, UserMessage},
    planner::{
        PlannerContext, PlannerPromptBuilder, SnapshotSummary, TargetSummary, ToolResultSummary,
    },
    session::AgentMessage,
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
            id: TargetId("local:macos".into()),
            platform: "macos".into(),
            capabilities: vec![
                "app_lifecycle".into(),
                "capture".into(),
                "inspect_tree".into(),
            ],
        },
        recent_tool_results: vec![ToolResultSummary {
            turn_index: 1,
            step_index: 1,
            tool_name: "observe".into(),
            is_error: false,
            read_only: true,
            summary: "snapshot snap-1 on frontmost (roots=1, elements=2, screenshot=capture-1.png)"
                .into(),
        }],
        latest_snapshot: Some(SnapshotSummary {
            id: "snap-1".into(),
            surface: "frontmost".into(),
            root_element_count: 1,
            element_count: 2,
            screenshot_artifact: Some(ArtifactId("capture-1.png".into())),
        }),
        previous_snapshot_visual: Some(ArtifactId("capture-prev.png".into())),
        notes: vec!["Observe again before finishing.".into()],
        ui_state_stale: true,
    }
}

#[test]
fn assemble_builds_json_first_prompt_contract_snapshot() {
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

    let context = builder.assemble(
        "Open Finder and confirm the window appears.",
        &planner_context(),
        &tools,
        &transcript,
    );

    let expected_custom_event = serde_json::to_string_pretty(&json!({
        "kind": "planner.feedback.v1",
        "payload": {
            "reason": "Need another observe."
        }
    }))
    .expect("custom transcript event should serialize");
    let expected_request = serde_json::to_string_pretty(&json!({
        "task": "Open Finder and confirm the window appears.",
        "target": {
            "id": "local:macos",
            "platform": "macos",
            "capabilities": [
                "app_lifecycle",
                "capture",
                "inspect_tree"
            ]
        },
        "recent_tool_results": [
            {
                "turn_index": 1,
                "step_index": 1,
                "tool_name": "observe",
                "is_error": false,
                "read_only": true,
                "summary": "snapshot snap-1 on frontmost (roots=1, elements=2, screenshot=capture-1.png)"
            }
        ],
        "latest_snapshot": {
            "id": "snap-1",
            "surface": "frontmost",
            "root_element_count": 1,
            "element_count": 2,
            "screenshot_artifact": "capture-1.png"
        },
        "previous_snapshot_visual": "capture-prev.png",
        "notes": [
            "Observe again before finishing."
        ],
        "ui_state_stale": true
    }))
    .expect("planner request should serialize");

    assert_eq!(
        serde_json::to_value(&context).expect("planner context should serialize"),
        json!({
            "system": "You are the Operator planner.\nChoose exactly one next decision for the current desktop automation task.\nUse only the provided tools and the transcript/context you are given.\nWhen `ui_state_stale` is true, call `observe` before any further side-effect tool.\nDo not finish while `ui_state_stale` is true.\nUse `observe` with `include_elements=true` whenever you need to verify UI content or state; screenshot-only or empty observations do not count as verification.\nDo not invent tool results, hidden UI state, or unsupported tool arguments.\nReturn exactly one JSON object and no surrounding prose.\nValid decision shapes:\n{\"decision\":\"call_tool\",\"name\":\"<tool-name>\",\"arguments\":{},\"summary\":\"<brief next-step summary>\",\"thought\":\"<optional reasoning>\"}\n{\"decision\":\"finish\",\"summary\":\"<why the task is complete>\"}\n{\"decision\":\"fail\",\"reason\":\"<why the task cannot continue>\"}",
            "messages": [
                {
                    "User": {
                        "content": [
                            {
                                "Text": {
                                    "text": "Open Finder."
                                }
                            }
                        ],
                        "timestamp_ms": 1
                    }
                },
                {
                    "Assistant": {
                        "content": [
                            {
                                "Text": {
                                    "text": "I will inspect the desktop first."
                                }
                            }
                        ],
                        "usage": {
                            "input_tokens": 0,
                            "output_tokens": 0,
                            "total_tokens": 0,
                            "cost": null
                        },
                        "stop": "Stop",
                        "error_message": null,
                        "timestamp_ms": 2
                    }
                },
                {
                    "User": {
                        "content": [
                            {
                                "Text": {
                                    "text": expected_custom_event
                                }
                            }
                        ],
                        "timestamp_ms": 0
                    }
                },
                {
                    "User": {
                        "content": [
                            {
                                "Text": {
                                    "text": expected_request
                                }
                            }
                        ],
                        "timestamp_ms": 0
                    }
                }
            ],
            "tools": [
                {
                    "name": "observe",
                    "description": "Capture a surface and persist the resulting snapshot.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "surface": { "type": "string" }
                        }
                    }
                },
                {
                    "name": "click",
                    "description": "Click a locator, coordinate, or target.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "x": { "type": "number" },
                            "y": { "type": "number" }
                        }
                    }
                }
            ]
        })
    );
}

#[test]
fn assemble_limits_recent_transcript_before_appending_current_request() {
    let builder = PlannerPromptBuilder::new().with_recent_message_limit(2);
    let transcript = vec![
        AgentMessage::from(user_message("earliest", 1)),
        AgentMessage::from(assistant_message("middle", 2)),
        AgentMessage::from(assistant_message("latest assistant", 3)),
        AgentMessage::custom("parser.feedback.v1", json!({ "error": "invalid json" })),
    ];

    let context = builder.assemble(
        "Retry with valid JSON.",
        &planner_context(),
        &[],
        &transcript,
    );
    let messages = serde_json::to_value(&context.messages).expect("messages should serialize");
    let expected_custom_event = serde_json::to_string_pretty(&json!({
        "kind": "parser.feedback.v1",
        "payload": {
            "error": "invalid json"
        }
    }))
    .expect("custom transcript event should serialize");

    assert_eq!(
        messages,
        json!([
            {
                "Assistant": {
                    "content": [
                        {
                            "Text": {
                                "text": "latest assistant"
                            }
                        }
                    ],
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "total_tokens": 0,
                        "cost": null
                    },
                    "stop": "Stop",
                    "error_message": null,
                    "timestamp_ms": 3
                }
            },
            {
                "User": {
                    "content": [
                        {
                            "Text": {
                                "text": expected_custom_event
                            }
                        }
                    ],
                    "timestamp_ms": 0
                }
            },
            {
                "User": {
                    "content": [
                        {
                            "Text": {
                                "text": serde_json::to_string_pretty(&json!({
                                    "task": "Retry with valid JSON.",
                                    "target": {
                                        "id": "local:macos",
                                        "platform": "macos",
                                        "capabilities": [
                                            "app_lifecycle",
                                            "capture",
                                            "inspect_tree"
                                        ]
                                    },
                                    "recent_tool_results": [
                                        {
                                            "turn_index": 1,
                                            "step_index": 1,
                                            "tool_name": "observe",
                                            "is_error": false,
                                            "read_only": true,
                                            "summary": "snapshot snap-1 on frontmost (roots=1, elements=2, screenshot=capture-1.png)"
                                        }
                                    ],
                                    "latest_snapshot": {
                                        "id": "snap-1",
                                        "surface": "frontmost",
                                        "root_element_count": 1,
                                        "element_count": 2,
                                        "screenshot_artifact": "capture-1.png"
                                    },
                                    "previous_snapshot_visual": "capture-prev.png",
                                    "notes": [
                                        "Observe again before finishing."
                                    ],
                                    "ui_state_stale": true
                                }))
                                .expect("planner request should serialize")
                            }
                        }
                    ],
                    "timestamp_ms": 0
                }
            }
        ])
    );
}
