mod chat_completions;
mod event;
mod provider;
mod registry;
mod responses;
mod types;

pub use chat_completions::ChatCompletionsProvider;
pub use event::{channel, DoneReason, ErrorReason, ModelEvent, ModelStream, ModelStreamWriter};
pub use provider::{HttpProviderConfig, ModelError, ModelProvider, ModelRequest};
pub use registry::{
    normalize_model_selector, EnvironmentProviderBootstrap, ModelRegistry,
    ModelRegistryBootstrapError, ResolvedModel, SelectedModelProviderConfig, CLI_MODEL_VALUES,
    DOUBAO_MODEL_ALIAS, DOUBAO_MODEL_SELECTOR, OPENAI_MODEL_ALIAS, OPENAI_MODEL_SELECTOR,
};
pub use responses::ResponsesProvider;
pub use types::{
    ApiKind, AssistantMessage, CallOptions, ContentBlock, Context, CoordinatePolicy, Cost, Message,
    ModelConfig, ModelId, ProviderKind, ReasoningLevel, ResponseFormat, StopReason,
    ToolResultMessage, ToolSpec, Usage, UserMessage,
};
