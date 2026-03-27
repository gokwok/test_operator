//! Runtime assembly and execution primitives for Operator.

mod builder;
mod config;
mod events;
mod platform_registry;
mod resolver;
mod runtime;
pub mod stores;
mod tool_registry;
pub mod tools;

pub use builder::RuntimeBuilder;
pub use config::{NamedTargetConfig, RuntimeConfig};
pub use events::{
    AuditEvent, AuditEventKind, EventSink, NullEventSink, Session, SessionEvent, SessionStatus,
};
pub use platform_registry::{PlatformDriverFactory, PlatformRegistry};
pub use resolver::TargetResolver;
pub use runtime::{Runtime, RuntimeCore};
pub use stores::{
    ArtifactStore, FileArtifactStore, FileSessionStore, FileSnapshotStore, NullSessionStore,
    SessionStore, SnapshotStore,
};
pub use tool_registry::{ToolHandler, ToolRegistration, ToolRegistry, ToolSpec};
