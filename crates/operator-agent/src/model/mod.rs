mod doubao;
mod event;
mod openai;
mod provider;
mod registry;
mod types;

pub use doubao::{DoubaoChatCompletionsProvider, DoubaoProviderConfig};
pub use event::{channel, DoneReason, ErrorReason, ModelEvent, ModelStream, ModelStreamWriter};
pub use openai::{OpenAiProviderConfig, OpenAiResponsesProvider};
pub use provider::{ModelError, ModelProvider, ModelRequest};
pub use registry::{
    normalize_model_selector, EnvironmentProviderBootstrap, ModelRegistry,
    ModelRegistryBootstrapError, ResolvedModel, SelectedModelProviderConfig, CLI_MODEL_VALUES,
    DOUBAO_MODEL_ALIAS, DOUBAO_MODEL_SELECTOR, OPENAI_MODEL_ALIAS, OPENAI_MODEL_SELECTOR,
};
pub use types::{
    AssistantMessage, CallOptions, ContentBlock, Context, CoordinatePolicy, Cost, Message,
    ModelConfig, ModelId, ProviderKind, ReasoningLevel, ResponseFormat, StopReason,
    ToolResultMessage, ToolSpec, Usage, UserMessage,
};
