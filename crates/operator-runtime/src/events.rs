use std::time::SystemTime;

use async_trait::async_trait;
use operator_core::{Capability, OperatorError, SessionId, TargetId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuditEvent {
    pub timestamp: SystemTime,
    pub session_id: Option<SessionId>,
    pub target_id: Option<TargetId>,
    pub kind: AuditEventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AuditEventKind {
    ToolInvoked {
        tool: String,
        input: serde_json::Value,
    },
    ToolCompleted {
        tool: String,
        duration_ms: u64,
        success: bool,
    },
    CapabilityDenied {
        tool: String,
        capability: Capability,
    },
    SideEffectBlocked {
        tool: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub created_at: SystemTime,
    pub task: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionEvent {
    UserInput {
        text: String,
    },
    ToolCall {
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        name: String,
        output: serde_json::Value,
    },
    ModelResponse {
        content: String,
    },
    Error {
        message: String,
    },
    Completed {
        summary: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: AuditEvent) -> Result<(), OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NullEventSink;

#[async_trait]
impl EventSink for NullEventSink {
    async fn emit(&self, _: AuditEvent) -> Result<(), OperatorError> {
        Ok(())
    }
}
