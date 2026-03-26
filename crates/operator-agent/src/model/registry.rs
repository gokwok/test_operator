use std::{collections::HashMap, sync::Arc};

use super::{
    doubao::{DoubaoChatCompletionsProvider, DoubaoProviderConfig},
    openai::{OpenAiProviderConfig, OpenAiResponsesProvider},
    provider::{ModelError, ModelProvider},
    types::{CallOptions, CoordinatePolicy, ModelConfig, ProviderKind, ReasoningLevel},
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelRegistryBootstrapError {
    #[error(
        "no model provider credentials found; set OPENAI_API_KEY, ARK_API_KEY, or DOUBAO_API_KEY"
    )]
    NoProviderCredentials,

    #[error(transparent)]
    ProviderInit(#[from] ModelError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentProviderBootstrap {
    pub openai: Option<OpenAiProviderConfig>,
    pub doubao: Option<DoubaoProviderConfig>,
}

impl EnvironmentProviderBootstrap {
    pub fn from_env() -> Result<Self, ModelRegistryBootstrapError> {
        Self::from_env_vars(current_env_vars())
    }

    pub fn from_env_vars<I, K, V>(vars: I) -> Result<Self, ModelRegistryBootstrapError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let vars = normalized_env_vars(vars);
        let openai = env_value(&vars, "OPENAI_API_KEY").map(|api_key| {
            let mut config = OpenAiProviderConfig::new(api_key.to_owned());
            if let Some(base_url) = env_value(&vars, "OPENAI_BASE_URL") {
                config.base_url = base_url.to_owned();
            }
            config
        });

        let doubao = first_env_value(&vars, &["ARK_API_KEY", "DOUBAO_API_KEY"]).map(|api_key| {
            let mut config = DoubaoProviderConfig::new(api_key.to_owned());
            if let Some(base_url) = first_env_value(&vars, &["ARK_BASE_URL", "DOUBAO_BASE_URL"]) {
                config.base_url = base_url.to_owned();
            }
            config
        });

        if openai.is_none() && doubao.is_none() {
            return Err(ModelRegistryBootstrapError::NoProviderCredentials);
        }

        Ok(Self { openai, doubao })
    }

    pub fn into_registry(self) -> Result<ModelRegistry, ModelRegistryBootstrapError> {
        let mut registry = ModelRegistry::new();
        self.register_into(&mut registry)?;
        Ok(registry)
    }

    pub fn register_into(
        &self,
        registry: &mut ModelRegistry,
    ) -> Result<(), ModelRegistryBootstrapError> {
        if let Some(config) = self.openai.clone() {
            let provider = OpenAiResponsesProvider::new(config)?;
            registry.register_provider(ProviderKind::OpenAi, Arc::new(provider));
        }

        if let Some(config) = self.doubao.clone() {
            let provider = DoubaoChatCompletionsProvider::new(config)?;
            registry.register_provider(ProviderKind::OpenAiCompatible, Arc::new(provider));
        }

        Ok(())
    }
}

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
            phase1_model(
                ProviderKind::OpenAiCompatible,
                "doubao-seed-2-0-lite-260215",
            ),
        );
        registry
    }

    pub fn from_environment() -> Result<Self, ModelRegistryBootstrapError> {
        EnvironmentProviderBootstrap::from_env()?.into_registry()
    }

    pub fn from_environment_vars<I, K, V>(vars: I) -> Result<Self, ModelRegistryBootstrapError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        EnvironmentProviderBootstrap::from_env_vars(vars)?.into_registry()
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

fn normalized_env_vars<I, K, V>(vars: I) -> HashMap<String, String>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    vars.into_iter()
        .filter_map(|(name, value)| {
            let value = value.as_ref().trim();
            if value.is_empty() {
                None
            } else {
                Some((name.as_ref().to_owned(), value.to_owned()))
            }
        })
        .collect()
}

fn current_env_vars() -> Vec<(&'static str, String)> {
    [
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "ARK_API_KEY",
        "ARK_BASE_URL",
        "DOUBAO_API_KEY",
        "DOUBAO_BASE_URL",
    ]
    .into_iter()
    .filter_map(|name| std::env::var(name).ok().map(|value| (name, value)))
    .collect()
}

fn env_value<'a>(vars: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    vars.get(name).map(String::as_str)
}

fn first_env_value<'a>(vars: &'a HashMap<String, String>, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| env_value(vars, name))
}

fn phase1_model(provider: ProviderKind, id: &'static str) -> ModelConfig {
    let reasoning_level = match id {
        "gpt-5.4" | "doubao-seed-2-0-lite-260215" => Some(ReasoningLevel::Minimal),
        _ => Some(ReasoningLevel::Medium),
    };
    let coordinate_policy = match id {
        "doubao-seed-2-0-lite-260215" => CoordinatePolicy::SurfaceNormalized1000,
        _ => CoordinatePolicy::ScreenAbsolutePixels,
    };

    ModelConfig {
        provider,
        id: Arc::from(id),
        coordinate_policy,
        default_options: CallOptions {
            temperature: None,
            max_output_tokens: None,
            reasoning_level,
            response_format: None,
        },
        default_timeout_ms: Some(30_000),
    }
}
