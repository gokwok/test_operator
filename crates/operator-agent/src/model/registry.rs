use std::{collections::HashMap, sync::Arc};

use super::{
    doubao::{DoubaoChatCompletionsProvider, DoubaoProviderConfig},
    openai::{OpenAiProviderConfig, OpenAiResponsesProvider},
    provider::{ModelError, ModelProvider},
    types::{CallOptions, CoordinatePolicy, ModelConfig, ProviderKind, ReasoningLevel},
};

pub const OPENAI_MODEL_SELECTOR: &str = "openai";
pub const DOUBAO_MODEL_SELECTOR: &str = "doubao";
pub const OPENAI_MODEL_ALIAS: &str = "gpt-5.4";
pub const DOUBAO_MODEL_ALIAS: &str = "doubao-seed";
pub const CLI_MODEL_VALUES: &[&str] = &[
    OPENAI_MODEL_SELECTOR,
    DOUBAO_MODEL_SELECTOR,
    OPENAI_MODEL_ALIAS,
    DOUBAO_MODEL_ALIAS,
];

const OPENAI_DEFAULT_MODEL_NAME: &str = "gpt-5.4";
const DOUBAO_DEFAULT_MODEL_NAME: &str = "doubao-seed-2-0-lite-260215";
const OPENAI_ENV_HINTS: &str = "OPENAI_API_KEY";
const DOUBAO_ENV_HINTS: &str = "ARK_API_KEY or DOUBAO_API_KEY";
const OPENAI_SELECTOR_NAMES: &[&str] = &[OPENAI_MODEL_SELECTOR, OPENAI_MODEL_ALIAS];
const DOUBAO_SELECTOR_NAMES: &[&str] = &[DOUBAO_MODEL_SELECTOR, DOUBAO_MODEL_ALIAS];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedModelProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelRegistryBootstrapError {
    #[error(
        "no model provider credentials found; set OPENAI_API_KEY, ARK_API_KEY, or DOUBAO_API_KEY"
    )]
    NoProviderCredentials,

    #[error("unsupported model selector `{0}`; expected one of: openai, doubao")]
    UnsupportedSelector(String),

    #[error(
        "no credentials configured for `{selector}`; set {env_hints} or configure [agent.model.provider.{selector}].api_key"
    )]
    MissingSelectedProviderCredentials {
        selector: String,
        env_hints: &'static str,
    },

    #[error(transparent)]
    ProviderInit(#[from] ModelError),
}

pub fn normalize_model_selector(name: &str) -> Result<&'static str, ModelRegistryBootstrapError> {
    match name.trim() {
        OPENAI_MODEL_SELECTOR | OPENAI_MODEL_ALIAS => Ok(OPENAI_MODEL_SELECTOR),
        DOUBAO_MODEL_SELECTOR | DOUBAO_MODEL_ALIAS => Ok(DOUBAO_MODEL_SELECTOR),
        other => Err(ModelRegistryBootstrapError::UnsupportedSelector(
            other.to_owned(),
        )),
    }
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
        registry.register_selector_model(
            OPENAI_MODEL_SELECTOR,
            ProviderKind::OpenAi,
            OPENAI_DEFAULT_MODEL_NAME,
        );
        registry.register_selector_model(
            DOUBAO_MODEL_SELECTOR,
            ProviderKind::OpenAiCompatible,
            DOUBAO_DEFAULT_MODEL_NAME,
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

    pub fn from_selected_provider_config(
        selector: &str,
        configured: &SelectedModelProviderConfig,
    ) -> Result<Self, ModelRegistryBootstrapError> {
        Self::from_selected_provider_config_and_env_vars(selector, configured, current_env_vars())
    }

    pub fn from_selected_provider_config_and_env_vars<I, K, V>(
        selector: &str,
        configured: &SelectedModelProviderConfig,
        vars: I,
    ) -> Result<Self, ModelRegistryBootstrapError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let selector = normalize_model_selector(selector)?;
        let vars = normalized_env_vars(vars);
        let mut registry = Self::new();

        match selector {
            OPENAI_MODEL_SELECTOR => {
                let api_key = normalized_option(configured.api_key.as_deref())
                    .or_else(|| env_value(&vars, "OPENAI_API_KEY").map(str::to_owned))
                    .ok_or_else(|| {
                        ModelRegistryBootstrapError::MissingSelectedProviderCredentials {
                            selector: selector.to_owned(),
                            env_hints: OPENAI_ENV_HINTS,
                        }
                    })?;
                let mut provider_config = OpenAiProviderConfig::new(api_key);
                if let Some(base_url) = normalized_option(configured.base_url.as_deref())
                    .or_else(|| env_value(&vars, "OPENAI_BASE_URL").map(str::to_owned))
                {
                    provider_config.base_url = base_url;
                }
                registry.register_provider(
                    ProviderKind::OpenAi,
                    Arc::new(OpenAiResponsesProvider::new(provider_config)?),
                );
                let model_name = normalized_option(configured.model_name.as_deref())
                    .unwrap_or_else(|| OPENAI_DEFAULT_MODEL_NAME.to_owned());
                registry.register_selector_model(selector, ProviderKind::OpenAi, &model_name);
            }
            DOUBAO_MODEL_SELECTOR => {
                let api_key = normalized_option(configured.api_key.as_deref())
                    .or_else(|| {
                        first_env_value(&vars, &["ARK_API_KEY", "DOUBAO_API_KEY"])
                            .map(str::to_owned)
                    })
                    .ok_or_else(|| {
                        ModelRegistryBootstrapError::MissingSelectedProviderCredentials {
                            selector: selector.to_owned(),
                            env_hints: DOUBAO_ENV_HINTS,
                        }
                    })?;
                let mut provider_config = DoubaoProviderConfig::new(api_key);
                if let Some(base_url) =
                    normalized_option(configured.base_url.as_deref()).or_else(|| {
                        first_env_value(&vars, &["ARK_BASE_URL", "DOUBAO_BASE_URL"])
                            .map(str::to_owned)
                    })
                {
                    provider_config.base_url = base_url;
                }
                registry.register_provider(
                    ProviderKind::OpenAiCompatible,
                    Arc::new(DoubaoChatCompletionsProvider::new(provider_config)?),
                );
                let model_name = normalized_option(configured.model_name.as_deref())
                    .unwrap_or_else(|| DOUBAO_DEFAULT_MODEL_NAME.to_owned());
                registry.register_selector_model(
                    selector,
                    ProviderKind::OpenAiCompatible,
                    &model_name,
                );
            }
            _ => unreachable!("normalize_model_selector only returns supported selectors"),
        }

        Ok(registry)
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

    fn register_selector_model(
        &mut self,
        selector: &str,
        provider: ProviderKind,
        model_name: &str,
    ) {
        let config = phase1_model(selector, provider, model_name);
        for name in selector_names(selector) {
            self.register_config(*name, config.clone());
        }
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

fn selector_names(selector: &str) -> &'static [&'static str] {
    match selector {
        OPENAI_MODEL_SELECTOR => OPENAI_SELECTOR_NAMES,
        DOUBAO_MODEL_SELECTOR => DOUBAO_SELECTOR_NAMES,
        _ => &[],
    }
}

fn env_value<'a>(vars: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    vars.get(name).map(String::as_str)
}

fn first_env_value<'a>(vars: &'a HashMap<String, String>, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| env_value(vars, name))
}

fn phase1_model(selector: &str, provider: ProviderKind, model_name: &str) -> ModelConfig {
    let reasoning_level = match selector {
        OPENAI_MODEL_SELECTOR | DOUBAO_MODEL_SELECTOR => Some(ReasoningLevel::Minimal),
        _ => Some(ReasoningLevel::Medium),
    };
    let coordinate_policy = match selector {
        DOUBAO_MODEL_SELECTOR => CoordinatePolicy::SurfaceNormalized1000,
        OPENAI_MODEL_SELECTOR => CoordinatePolicy::SurfaceImagePixels,
        _ => CoordinatePolicy::ScreenAbsolutePixels,
    };

    ModelConfig {
        provider,
        id: Arc::from(model_name),
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

fn normalized_option(value: Option<&str>) -> Option<String> {
    value.and_then(normalized_optional_string)
}

fn normalized_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
