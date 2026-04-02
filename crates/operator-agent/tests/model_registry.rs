mod support;

use std::{sync::Arc, time::Duration};

use operator_agent::model::{
    normalize_model_selector, ApiKind, CallOptions, ContentBlock, Context, CoordinatePolicy,
    EnvironmentProviderBootstrap, HttpProviderConfig, Message, ModelConfig, ModelError, ModelEvent,
    ModelRegistry, ModelRegistryBootstrapError, ModelRequest, ProviderKind, ReasoningLevel,
    SelectedModelProviderConfig, ToolSpec, UserMessage,
};
use serde_json::json;
use support::DeterministicTestProvider;

#[test]
fn default_registry_exposes_selector_models_and_compatibility_aliases() {
    let registry = ModelRegistry::new();

    let openai = registry
        .config("openai")
        .expect("openai selector should exist");
    assert_eq!(openai.provider, ProviderKind::OpenAi);
    assert_eq!(openai.id.as_ref(), "gpt-5.4");
    assert_eq!(openai.default_timeout_ms, Some(30_000));
    assert_eq!(
        openai.default_options.reasoning_level,
        Some(ReasoningLevel::Minimal)
    );
    assert_eq!(
        openai.coordinate_policy,
        CoordinatePolicy::SurfaceImagePixels
    );
    assert_eq!(
        registry.config("gpt-5.4"),
        Some(openai),
        "openai alias should resolve to the canonical selector config"
    );

    let doubao = registry
        .config("doubao")
        .expect("doubao selector should exist");
    assert_eq!(doubao.provider, ProviderKind::Doubao);
    assert_eq!(doubao.api_kind, ApiKind::ChatCompletions);
    assert_eq!(doubao.id.as_ref(), "doubao-seed-2-0-lite-260215");
    assert_eq!(doubao.default_timeout_ms, Some(30_000));
    assert_eq!(
        doubao.coordinate_policy,
        CoordinatePolicy::SurfaceNormalized1000
    );
    assert_eq!(
        doubao.default_options.reasoning_level,
        Some(ReasoningLevel::Minimal)
    );
    assert_eq!(
        registry.config("doubao-seed"),
        Some(doubao),
        "doubao alias should resolve to the canonical selector config"
    );
}

#[test]
fn resolve_reports_missing_provider_for_registered_model() {
    let registry = ModelRegistry::new();

    assert!(matches!(
        registry.resolve("openai"),
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
        .resolve("openai")
        .expect("registered provider should resolve");
    assert_eq!(
        resolved.config,
        ModelConfig {
            provider: ProviderKind::OpenAi,
            api_kind: ApiKind::Responses,
            id: Arc::from("gpt-5.4"),
            coordinate_policy: CoordinatePolicy::SurfaceImagePixels,
            default_options: CallOptions {
                temperature: None,
                max_output_tokens: None,
                reasoning_level: Some(ReasoningLevel::Minimal),
                response_format: None,
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

#[test]
fn environment_provider_bootstrap_reads_supported_credentials_from_env_vars() {
    let bootstrap = EnvironmentProviderBootstrap::from_env_vars([
        ("OPENAI_API_KEY", " openai-key "),
        ("OPENAI_BASE_URL", " https://openai.example/v1 "),
        ("ARK_API_KEY", " ark-key "),
        ("ARK_BASE_URL", " https://ark.example/api/v3 "),
        ("DOUBAO_API_KEY", " doubao-fallback "),
        ("DOUBAO_BASE_URL", " https://doubao.example/api/v3 "),
    ])
    .expect("supported provider credentials should bootstrap");

    assert_eq!(
        bootstrap.openai,
        Some(HttpProviderConfig {
            provider: ProviderKind::OpenAi,
            api_key: "openai-key".into(),
            base_url: "https://openai.example/v1".into(),
        })
    );
    assert_eq!(
        bootstrap.doubao,
        Some(HttpProviderConfig {
            provider: ProviderKind::Doubao,
            api_key: "ark-key".into(),
            base_url: "https://ark.example/api/v3".into(),
        })
    );
}

#[test]
fn environment_provider_bootstrap_rejects_missing_credentials() {
    assert!(matches!(
        EnvironmentProviderBootstrap::from_env_vars([
            ("OPENAI_API_KEY", "   "),
            ("ARK_API_KEY", ""),
            ("DOUBAO_API_KEY", " "),
        ]),
        Err(ModelRegistryBootstrapError::NoProviderCredentials)
    ));
}

#[test]
fn environment_bootstrap_registers_configured_providers_into_model_registry() {
    let registry = ModelRegistry::from_environment_vars([
        ("OPENAI_API_KEY", "openai-key"),
        ("DOUBAO_API_KEY", "doubao-key"),
    ])
    .expect("environment bootstrap should register supported providers");

    assert!(registry.resolve("openai").is_ok());
    assert!(registry.resolve("doubao").is_ok());
    assert!(registry.resolve("gpt-5.4").is_ok());
    assert!(registry.resolve("doubao-seed").is_ok());
}

#[test]
fn selector_normalization_maps_compatibility_aliases_to_stable_selectors() {
    assert_eq!(normalize_model_selector("openai").unwrap(), "openai");
    assert_eq!(normalize_model_selector("gpt-5.4").unwrap(), "openai");
    assert_eq!(normalize_model_selector("doubao").unwrap(), "doubao");
    assert_eq!(normalize_model_selector("doubao-seed").unwrap(), "doubao");
}

#[test]
fn selector_bootstrap_uses_config_values_and_keeps_alias_mapping_explicit() {
    let registry = ModelRegistry::from_selected_provider_config_and_env_vars(
        "gpt-5.4",
        &SelectedModelProviderConfig {
            api_key: Some("sk-openai".into()),
            base_url: Some("https://openai.internal/v1".into()),
            model_name: Some("gpt-5.4-mini".into()),
            api_kind: None,
        },
        std::iter::empty::<(&str, &str)>(),
    )
    .expect("config-backed bootstrap should resolve the canonical selector");

    let selector = registry.resolve("openai").expect("openai should resolve");
    assert_eq!(selector.config.id.as_ref(), "gpt-5.4-mini");
    assert_eq!(selector.config.api_kind, ApiKind::Responses);
    assert_eq!(
        selector.config.coordinate_policy,
        CoordinatePolicy::SurfaceImagePixels
    );

    let alias = registry
        .resolve("gpt-5.4")
        .expect("compatibility alias should resolve");
    assert_eq!(
        alias.config.id, selector.config.id,
        "alias should map to the same provider-side model id as the stable selector"
    );
}

#[test]
fn selector_bootstrap_falls_back_to_environment_fields_for_missing_values() {
    let registry = ModelRegistry::from_selected_provider_config_and_env_vars(
        "doubao",
        &SelectedModelProviderConfig {
            api_key: None,
            base_url: None,
            model_name: Some("doubao-pro-32k".into()),
            api_kind: None,
        },
        [
            ("DOUBAO_API_KEY", "doubao-env"),
            ("DOUBAO_BASE_URL", "https://doubao.internal/api/v3"),
        ],
    )
    .expect("env fallback should bootstrap the selected provider");

    let resolved = registry.resolve("doubao").expect("doubao should resolve");
    assert_eq!(resolved.config.id.as_ref(), "doubao-pro-32k");
    assert_eq!(resolved.config.api_kind, ApiKind::ChatCompletions);
    assert_eq!(
        resolved.config.coordinate_policy,
        CoordinatePolicy::SurfaceNormalized1000
    );
}

#[test]
fn selector_bootstrap_rejects_missing_credentials_for_selected_provider() {
    let result = ModelRegistry::from_selected_provider_config_and_env_vars(
        "openai",
        &SelectedModelProviderConfig::default(),
        std::iter::empty::<(&str, &str)>(),
    );

    assert!(matches!(
        result,
        Err(ModelRegistryBootstrapError::MissingSelectedProviderCredentials { selector, .. })
        if selector == "openai"
    ));
}
