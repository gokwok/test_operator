use std::{num::NonZeroUsize, sync::Arc};

use tokio::sync::{mpsc, oneshot};

use super::{provider::ModelError, types::AssistantMessage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoneReason {
    Stop,
    Length,
    ToolUse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorReason {
    Aborted,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModelEvent {
    Start,
    TextStart {
        content_index: usize,
    },
    TextDelta {
        content_index: usize,
        delta: String,
    },
    TextEnd {
        content_index: usize,
    },
    ThinkingStart {
        content_index: usize,
    },
    ThinkingDelta {
        content_index: usize,
        delta: String,
    },
    ThinkingEnd {
        content_index: usize,
    },
    ToolCallStart {
        content_index: usize,
        tool_call_id: Arc<str>,
        tool_name: Option<Arc<str>>,
    },
    ToolCallDelta {
        content_index: usize,
        tool_call_id: Arc<str>,
        tool_name: Option<Arc<str>>,
        arguments_delta: String,
    },
    ToolCallEnd {
        content_index: usize,
        tool_call_id: Arc<str>,
    },
    Done {
        reason: DoneReason,
        message: AssistantMessage,
    },
    Error {
        reason: ErrorReason,
        error: ModelError,
    },
}

#[derive(Debug)]
pub struct ModelStream {
    events: mpsc::Receiver<ModelEvent>,
    result: oneshot::Receiver<Result<AssistantMessage, ModelError>>,
}

impl ModelStream {
    pub async fn recv(&mut self) -> Option<ModelEvent> {
        self.events.recv().await
    }

    pub async fn result(self) -> Result<AssistantMessage, ModelError> {
        self.result
            .await
            .map_err(|_| ModelError::Protocol("model stream result channel dropped".to_owned()))?
    }
}

#[derive(Debug)]
pub struct ModelStreamWriter {
    events: mpsc::Sender<ModelEvent>,
    result: Option<oneshot::Sender<Result<AssistantMessage, ModelError>>>,
}

impl ModelStreamWriter {
    pub async fn emit(&self, event: ModelEvent) -> bool {
        self.events.send(event).await.is_ok()
    }

    pub fn finish(mut self, result: Result<AssistantMessage, ModelError>) -> bool {
        if let Some(tx) = self.result.take() {
            return tx.send(result).is_ok();
        }

        false
    }

    pub fn is_closed(&self) -> bool {
        self.events.is_closed()
    }
}

impl Drop for ModelStreamWriter {
    fn drop(&mut self) {
        if let Some(tx) = self.result.take() {
            let _ = tx.send(Err(ModelError::Aborted));
        }
    }
}

pub fn channel(event_capacity: NonZeroUsize) -> (ModelStream, ModelStreamWriter) {
    let (event_tx, event_rx) = mpsc::channel(event_capacity.get());
    let (result_tx, result_rx) = oneshot::channel();

    (
        ModelStream {
            events: event_rx,
            result: result_rx,
        },
        ModelStreamWriter {
            events: event_tx,
            result: Some(result_tx),
        },
    )
}
