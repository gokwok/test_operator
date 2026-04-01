use operator_agent::{
    planner::{AgentDecision, DecisionValidator},
    tools::{AgentToolSpec, ToolCatalogOptions, ToolExecutor},
};
use operator_core::{Capability, CapabilitySet, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};
use serde_json::json;
use std::sync::Arc;

fn tool_specs() -> Vec<AgentToolSpec> {
    vec![
        AgentToolSpec {
            name: "observe".into(),
            description: "Capture the current UI state.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["surface"],
                "properties": {
                    "surface": { "type": "string", "enum": ["frontmost", "fullscreen"] },
                    "capture": { "type": ["string", "null"] }
                }
            }),
            read_only: true,
        },
        AgentToolSpec {
            name: "click".into(),
            description: "Click a coordinate.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["x", "y"],
                "properties": {
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "button": { "type": "string", "enum": ["left", "right"] }
                }
            }),
            read_only: false,
        },
    ]
}

#[test]
fn accepts_decisions_that_match_an_available_tool_schema() {
    let validator = DecisionValidator::new(&tool_specs());
    let decision = AgentDecision::CallTool {
        name: "observe".into(),
        arguments: json!({
            "surface": "frontmost",
            "capture": null
        }),
        summary: "Inspect the frontmost UI.".into(),
        thought: Some("Need fresh state before acting.".into()),
    };

    validator
        .validate(&decision)
        .expect("matching tool decision should validate");
}

#[test]
fn rejects_unknown_tools() {
    let validator = DecisionValidator::new(&tool_specs());
    let decision = AgentDecision::CallTool {
        name: "type".into(),
        arguments: json!({
            "text": "hello"
        }),
        summary: "Type into the focused field.".into(),
        thought: None,
    };

    let error = validator
        .validate(&decision)
        .expect_err("unknown tools must be rejected");

    assert!(
        error.to_string().contains("not available"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_non_object_tool_arguments() {
    let validator = DecisionValidator::new(&tool_specs());
    let decision = AgentDecision::CallTool {
        name: "observe".into(),
        arguments: json!(["frontmost"]),
        summary: "Inspect the frontmost UI.".into(),
        thought: None,
    };

    let error = validator
        .validate(&decision)
        .expect_err("non-object arguments must be rejected");

    assert!(
        error.to_string().contains("JSON object"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_arguments_that_fail_required_or_type_checks() {
    let validator = DecisionValidator::new(&tool_specs());
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "x": "10"
        }),
        summary: "Click the primary button.".into(),
        thought: None,
    };

    let error = validator
        .validate(&decision)
        .expect_err("schema violations must be rejected");

    assert!(
        error.to_string().contains("schema"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_side_effect_tools_absent_from_the_filtered_catalog() {
    let validator = DecisionValidator::new(&tool_specs()[0..1]);
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "x": 10,
            "y": 20
        }),
        summary: "Click the primary button.".into(),
        thought: None,
    };

    let error = validator
        .validate(&decision)
        .expect_err("tools outside the filtered catalog must be rejected");

    assert!(
        error.to_string().contains("not available"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn validates_against_runtime_generated_tool_schema() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");
    let executor = ToolExecutor::new(runtime.core(), runtime.tools().clone());
    let catalog = executor
        .catalog(&TargetId("macos".into()))
        .expect("catalog should resolve");
    let validator = DecisionValidator::new(&catalog);
    let decision = AgentDecision::CallTool {
        name: "observe".into(),
        arguments: json!({
            "surface": {
                "kind": "Frontmost"
            },
            "include_screenshot": true,
            "include_elements": true
        }),
        summary: "Capture the current frontmost surface.".into(),
        thought: None,
    };

    validator
        .validate(&decision)
        .expect("runtime-generated observe schema should validate");
}

#[tokio::test]
async fn rejects_selector_locators_when_catalog_prunes_them() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::PointerInput]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");
    let executor = ToolExecutor::new(runtime.core(), runtime.tools().clone());
    let catalog = executor
        .catalog_with_options(
            &TargetId("macos".into()),
            ToolCatalogOptions {
                allow_selector_locators: false,
            },
        )
        .expect("catalog should resolve");
    let validator = DecisionValidator::new(&catalog);
    let decision = AgentDecision::CallTool {
        name: "click".into(),
        arguments: json!({
            "locator": {
                "Text": "Submit"
            }
        }),
        summary: "Click the Submit control by its text.".into(),
        thought: None,
    };

    let error = validator
        .validate(&decision)
        .expect_err("selector locators should be rejected after pruning");
    assert!(
        error.to_string().contains("schema"),
        "unexpected error: {error}"
    );
}
