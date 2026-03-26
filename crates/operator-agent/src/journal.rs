use operator_core::SessionId;
use operator_runtime::{Session, SessionEvent, SessionStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{tools::AgentToolResult, AgentError};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ReplayableTranscriptEvent {
    UserInput { text: String },
    ToolCall { name: String, input: Value },
    ToolResult { result: AgentToolResult },
    ModelResponse { content: String },
    Completed { summary: Option<String> },
    Error { message: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedSessionTranscript {
    pub session: Session,
    pub events: Vec<ReplayableTranscriptEvent>,
}

#[derive(Debug, Clone)]
pub struct SessionJournal {
    session_id: SessionId,
    pending: Vec<SessionEvent>,
}

impl SessionJournal {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            pending: Vec::new(),
        }
    }

    pub fn record(&mut self, event: SessionEvent) {
        self.pending.push(event);
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub async fn flush(
        &mut self,
        store: &dyn SessionStore,
    ) -> Result<(), operator_core::OperatorError> {
        let mut pending = std::mem::take(&mut self.pending).into_iter();
        while let Some(event) = pending.next() {
            if let Err(error) = store.append(&self.session_id, &event).await {
                self.pending.push(event);
                self.pending.extend(pending);
                return Err(error);
            }
        }

        Ok(())
    }
}

impl TryFrom<SessionEvent> for ReplayableTranscriptEvent {
    type Error = operator_core::OperatorError;

    fn try_from(value: SessionEvent) -> Result<Self, operator_core::OperatorError> {
        match value {
            SessionEvent::UserInput { text } => Ok(Self::UserInput { text }),
            SessionEvent::ToolCall { name, input } => Ok(Self::ToolCall { name, input }),
            SessionEvent::ToolResult { output, .. } => Ok(Self::ToolResult {
                result: serde_json::from_value(output)?,
            }),
            SessionEvent::ModelResponse { content } => Ok(Self::ModelResponse { content }),
            SessionEvent::Completed { summary } => Ok(Self::Completed { summary }),
            SessionEvent::Error { message } => Ok(Self::Error { message }),
        }
    }
}

pub async fn load_persisted_session(
    store: &dyn SessionStore,
    id: &SessionId,
) -> Result<Option<PersistedSessionTranscript>, AgentError> {
    let Some(session) = store.get(id).await? else {
        return Ok(None);
    };
    let events = store
        .events(id)
        .await?
        .into_iter()
        .map(ReplayableTranscriptEvent::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(PersistedSessionTranscript { session, events }))
}
