mod support;

use std::{sync::Arc, time::Duration};

use operator_agent::model::{
    CallOptions, ContentBlock, Context, Message, ModelConfig, ModelError, ModelEvent,
    ModelRegistry, ModelRequest, ProviderKind, ReasoningLevel, ToolSpec, UserMessage,
};
use serde_json::json;
use support::DeterministicTestProvider;

#[test]
fn default_registry_exposes_phase1_models() {
    let registry = ModelRegistry::new();

    let gpt = registry.config("gpt-5.4").expect("gpt-5.4 should exist");
    assert_eq!(gpt.provider, ProviderKind::OpenAi);
    assert_eq!(gpt.id.as_ref(), "gpt-5.4");
    assert_eq!(gpt.default_timeout_ms, Some(30_000));
    assert_eq!(
        gpt.default_options.reasoning_level,
        Some(ReasoningLevel::Medium)
    );

    let doubao = registry
        .config("doubao-seed")
        .expect("doubao-seed should exist");
    assert_eq!(doubao.provider, ProviderKind::OpenAiCompatible);
    assert_eq!(doubao.id.as_ref(), "doubao-seed");
    assert_eq!(doubao.default_timeout_ms, Some(30_000));
}

#[test]
fn resolve_reports_missing_provider_for_registered_model() {
    let registry = ModelRegistry::new();

    assert!(matches!(
        registry.resolve("gpt-5.4"),
        Err(ModelError::ProviderNotFound {
            provider: ProviderKind::OpenAi
        })
    ));
}

#[tokio::test]
async fn resolve_returns_registered_provider_for_known_model() {
    let mut registry = ModelRegistry::new();
    registry.register_provider(
        ProviderKind::OpenAi,
        Arc::new(DeterministicTestProvider::new("planned next step")),
    );

    let resolved = registry
        .resolve("gpt-5.4")
        .expect("registered provider should resolve");
    assert_eq!(
        resolved.config,
        ModelConfig {
            provider: ProviderKind::OpenAi,
            id: Arc::from("gpt-5.4"),
            default_options: CallOptions {
                temperature: None,
                max_output_tokens: None,
                reasoning_level: Some(ReasoningLevel::Medium),
            },
            default_timeout_ms: Some(30_000),
        }
    );

    let request = ModelRequest {
        config: resolved.config.clone(),
        context: Context {
            system: Some("You are Operator.".into()),
            messages: vec![Message::User(UserMessage {
                content: vec![ContentBlock::Text {
                    text: "Open Safari.".into(),
                }],
                timestamp_ms: 0,
            })],
            tools: vec![ToolSpec {
                name: Arc::from("observe"),
                description: Arc::from("Capture the frontmost surface."),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "surface": { "type": "string" }
                    }
                }),
            }],
        },
        options: resolved.config.default_options.clone(),
        stream: false,
        timeout: Some(Duration::from_secs(30)),
        request_id: Some(Arc::from("req-1")),
        max_retry_delay_ms: Some(250),
    };

    let mut stream = resolved.provider.stream(request);
    assert!(matches!(stream.recv().await, Some(ModelEvent::Start)));

    let message = stream
        .result()
        .await
        .expect("deterministic provider should finish");
    assert_eq!(
        message.content,
        vec![ContentBlock::Text {
            text: "planned next step".into(),
        }]
    );
}
