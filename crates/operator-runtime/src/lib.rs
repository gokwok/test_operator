//! Runtime assembly and execution primitives for Operator.

mod builder;
mod config;
mod events;
mod resolver;
pub mod stores;

pub use builder::{Runtime, RuntimeBuilder, RuntimeCore};
pub use config::RuntimeConfig;
pub use events::{
    AuditEvent, AuditEventKind, EventSink, NullEventSink, Session, SessionEvent, SessionStatus,
};
pub use resolver::TargetResolver;
pub use stores::{
    FileSessionStore, FileSnapshotStore, NullSessionStore, SessionStore, SnapshotStore,
};
