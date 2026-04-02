use std::{sync::Arc, time::Duration};

use super::{
    event::ModelStream,
    types::{CallOptions, Context, ModelConfig, ProviderKind},
};

#[derive(Clone, Debug, PartialEq)]
pub struct ModelRequest {
    pub config: ModelConfig,
    pub context: Context,
    pub options: CallOptions,
    pub stream: bool,
    pub timeout: Option<Duration>,
    pub request_id: Option<Arc<str>>,
    pub max_retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("model request timeout")]
    Timeout,

    #[error("model request aborted")]
    Aborted,

    #[error("provider not found: {provider:?}")]
    ProviderNotFound { provider: ProviderKind },

    #[error("provider init failed ({provider:?}): {message}")]
    ProviderInitFailed {
        provider: ProviderKind,
        message: String,
    },

    #[error("transport error: {0}")]
    Transport(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("provider error: {0}")]
    Provider(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpProviderConfig {
    pub provider: ProviderKind,
    pub api_key: String,
    pub base_url: String,
}

pub trait ModelProvider: Send + Sync + 'static {
    fn stream(&self, req: ModelRequest) -> ModelStream;
}
