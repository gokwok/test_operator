use std::{collections::HashMap, sync::Arc};

use super::{
    chat_completions::ChatCompletionsProvider,
    provider::{HttpProviderConfig, ModelError, ModelProvider},
    responses::ResponsesProvider,
    types::{ApiKind, CallOptions, CoordinatePolicy, ModelConfig, ProviderKind, ReasoningLevel},
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
const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DOUBAO_DEFAULT_BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/v3";
const OPENAI_ENV_HINTS: &str = "OPENAI_API_KEY";
const DOUBAO_ENV_HINTS: &str = "ARK_API_KEY or DOUBAO_API_KEY";
const OPENAI_SELECTOR_NAMES: &[&str] = &[OPENAI_MODEL_SELECTOR, OPENAI_MODEL_ALIAS];
const DOUBAO_SELECTOR_NAMES: &[&str] = &[DOUBAO_MODEL_SELECTOR, DOUBAO_MODEL_ALIAS];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedModelProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
    pub api_kind: Option<String>,
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
        "unsupported api_kind `{api_kind}` for `{selector}`; expected one of: responses, chat_completions"
    )]
    UnsupportedApiKind { selector: String, api_kind: String },

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
    pub openai: Option<HttpProviderConfig>,
    pub doubao: Option<HttpProviderConfig>,
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
        let openai = env_value(&vars, "OPENAI_API_KEY").map(|api_key| HttpProviderConfig {
            provider: ProviderKind::OpenAi,
            api_key: api_key.to_owned(),
            base_url: env_value(&vars, "OPENAI_BASE_URL")
                .unwrap_or(OPENAI_DEFAULT_BASE_URL)
                .to_owned(),
        });

        let doubao = first_env_value(&vars, &["ARK_API_KEY", "DOUBAO_API_KEY"]).map(|api_key| {
            HttpProviderConfig {
                provider: ProviderKind::Doubao,
                api_key: api_key.to_owned(),
                base_url: first_env_value(&vars, &["ARK_BASE_URL", "DOUBAO_BASE_URL"])
                    .unwrap_or(DOUBAO_DEFAULT_BASE_URL)
                    .to_owned(),
            }
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
            registry.register_provider(
                ProviderKind::OpenAi,
                build_provider(default_api_kind_for_provider(ProviderKind::OpenAi), config)?,
            );
        }

        if let Some(config) = self.doubao.clone() {
            registry.register_provider(
                ProviderKind::Doubao,
                build_provider(default_api_kind_for_provider(ProviderKind::Doubao), config)?,
            );
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
            ApiKind::Responses,
            OPENAI_DEFAULT_MODEL_NAME,
        );
        registry.register_selector_model(
            DOUBAO_MODEL_SELECTOR,
            ProviderKind::Doubao,
            ApiKind::ChatCompletions,
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

        let provider = provider_for_selector(selector);
        let api_key = selected_api_key(selector, configured, &vars)?;
        let base_url = selected_base_url(selector, configured, &vars);
        let api_kind = selected_api_kind(selector, configured)?;
        let model_name = normalized_option(configured.model_name.as_deref())
            .unwrap_or_else(|| default_model_name(selector).to_owned());

        registry.register_provider(
            provider,
            build_provider(
                api_kind,
                HttpProviderConfig {
                    provider,
                    api_key,
                    base_url,
                },
            )?,
        );
        registry.register_selector_model(selector, provider, api_kind, &model_name);

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
        api_kind: ApiKind,
        model_name: &str,
    ) {
        let config = phase1_model(selector, provider, api_kind, model_name);
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

fn default_model_name(selector: &str) -> &'static str {
    match selector {
        OPENAI_MODEL_SELECTOR => OPENAI_DEFAULT_MODEL_NAME,
        DOUBAO_MODEL_SELECTOR => DOUBAO_DEFAULT_MODEL_NAME,
        _ => OPENAI_DEFAULT_MODEL_NAME,
    }
}

fn default_api_kind_for_provider(provider: ProviderKind) -> ApiKind {
    match provider {
        ProviderKind::OpenAi => ApiKind::Responses,
        ProviderKind::Doubao => ApiKind::ChatCompletions,
    }
}

fn default_api_kind_for_selector(selector: &str) -> ApiKind {
    default_api_kind_for_provider(provider_for_selector(selector))
}

fn provider_for_selector(selector: &str) -> ProviderKind {
    match selector {
        OPENAI_MODEL_SELECTOR => ProviderKind::OpenAi,
        DOUBAO_MODEL_SELECTOR => ProviderKind::Doubao,
        _ => ProviderKind::OpenAi,
    }
}

fn default_base_url(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => OPENAI_DEFAULT_BASE_URL,
        ProviderKind::Doubao => DOUBAO_DEFAULT_BASE_URL,
    }
}

fn selected_api_key(
    selector: &str,
    configured: &SelectedModelProviderConfig,
    vars: &HashMap<String, String>,
) -> Result<String, ModelRegistryBootstrapError> {
    match selector {
        OPENAI_MODEL_SELECTOR => normalized_option(configured.api_key.as_deref())
            .or_else(|| env_value(vars, "OPENAI_API_KEY").map(str::to_owned))
            .ok_or_else(
                || ModelRegistryBootstrapError::MissingSelectedProviderCredentials {
                    selector: selector.to_owned(),
                    env_hints: OPENAI_ENV_HINTS,
                },
            ),
        DOUBAO_MODEL_SELECTOR => normalized_option(configured.api_key.as_deref())
            .or_else(|| {
                first_env_value(vars, &["ARK_API_KEY", "DOUBAO_API_KEY"]).map(str::to_owned)
            })
            .ok_or_else(
                || ModelRegistryBootstrapError::MissingSelectedProviderCredentials {
                    selector: selector.to_owned(),
                    env_hints: DOUBAO_ENV_HINTS,
                },
            ),
        _ => unreachable!("normalize_model_selector only returns supported selectors"),
    }
}

fn selected_base_url(
    selector: &str,
    configured: &SelectedModelProviderConfig,
    vars: &HashMap<String, String>,
) -> String {
    match selector {
        OPENAI_MODEL_SELECTOR => normalized_option(configured.base_url.as_deref())
            .or_else(|| env_value(vars, "OPENAI_BASE_URL").map(str::to_owned))
            .unwrap_or_else(|| default_base_url(ProviderKind::OpenAi).to_owned()),
        DOUBAO_MODEL_SELECTOR => normalized_option(configured.base_url.as_deref())
            .or_else(|| {
                first_env_value(vars, &["ARK_BASE_URL", "DOUBAO_BASE_URL"]).map(str::to_owned)
            })
            .unwrap_or_else(|| default_base_url(ProviderKind::Doubao).to_owned()),
        _ => unreachable!("normalize_model_selector only returns supported selectors"),
    }
}

fn selected_api_kind(
    selector: &str,
    configured: &SelectedModelProviderConfig,
) -> Result<ApiKind, ModelRegistryBootstrapError> {
    Ok(configured
        .api_kind
        .as_deref()
        .map(|api_kind| normalize_api_kind(selector, api_kind))
        .transpose()?
        .unwrap_or_else(|| default_api_kind_for_selector(selector)))
}

fn normalize_api_kind(
    selector: &str,
    api_kind: &str,
) -> Result<ApiKind, ModelRegistryBootstrapError> {
    match api_kind.trim() {
        "responses" => Ok(ApiKind::Responses),
        "chat_completions" => Ok(ApiKind::ChatCompletions),
        other => Err(ModelRegistryBootstrapError::UnsupportedApiKind {
            selector: selector.to_owned(),
            api_kind: other.to_owned(),
        }),
    }
}

fn build_provider(
    api_kind: ApiKind,
    config: HttpProviderConfig,
) -> Result<Arc<dyn ModelProvider>, ModelRegistryBootstrapError> {
    let provider: Arc<dyn ModelProvider> = match api_kind {
        ApiKind::Responses => Arc::new(ResponsesProvider::new(config)?),
        ApiKind::ChatCompletions => Arc::new(ChatCompletionsProvider::new(config)?),
    };
    Ok(provider)
}

fn phase1_model(
    selector: &str,
    provider: ProviderKind,
    api_kind: ApiKind,
    model_name: &str,
) -> ModelConfig {
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
        api_kind,
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
