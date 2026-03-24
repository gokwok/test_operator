use std::{collections::HashMap, sync::Arc};

use super::{
    provider::{ModelError, ModelProvider},
    types::{CallOptions, ModelConfig, ProviderKind, ReasoningLevel},
};

#[derive(Clone)]
pub struct ResolvedModel {
    pub config: ModelConfig,
    pub provider: Arc<dyn ModelProvider>,
}

#[derive(Default)]
pub struct ModelRegistry {
    configs: HashMap<Arc<str>, ModelConfig>,
    providers: HashMap<ProviderKind, Arc<dyn ModelProvider>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_config("gpt-5.4", phase1_model(ProviderKind::OpenAi, "gpt-5.4"));
        registry.register_config(
            "doubao-seed",
            phase1_model(ProviderKind::OpenAiCompatible, "doubao-seed"),
        );
        registry
    }

    pub fn config(&self, name: &str) -> Option<&ModelConfig> {
        self.configs.get(name)
    }

    pub fn register_config(
        &mut self,
        name: impl Into<Arc<str>>,
        config: ModelConfig,
    ) -> Option<ModelConfig> {
        self.configs.insert(name.into(), config)
    }

    pub fn register_provider(
        &mut self,
        provider: ProviderKind,
        implementation: Arc<dyn ModelProvider>,
    ) -> Option<Arc<dyn ModelProvider>> {
        self.providers.insert(provider, implementation)
    }

    pub fn provider(&self, provider: ProviderKind) -> Option<Arc<dyn ModelProvider>> {
        self.providers.get(&provider).cloned()
    }

    pub fn resolve(&self, name: &str) -> Result<ResolvedModel, ModelError> {
        let config = self
            .config(name)
            .cloned()
            .ok_or_else(|| ModelError::ModelNotFound(name.to_owned()))?;
        let provider = self
            .provider(config.provider)
            .ok_or(ModelError::ProviderNotFound {
                provider: config.provider,
            })?;

        Ok(ResolvedModel { config, provider })
    }
}

fn phase1_model(provider: ProviderKind, id: &'static str) -> ModelConfig {
    let reasoning_level = match id {
        "gpt-5.4" => Some(ReasoningLevel::Minimal),
        _ => Some(ReasoningLevel::Medium),
    };

    ModelConfig {
        provider,
        id: Arc::from(id),
        default_options: CallOptions {
            temperature: None,
            max_output_tokens: None,
            reasoning_level,
            response_format: None,
        },
        default_timeout_ms: Some(30_000),
    }
}
