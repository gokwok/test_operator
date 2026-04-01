use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use operator_agent::model::{
    channel, AssistantMessage, ContentBlock, DoneReason, ModelError, ModelEvent, ModelProvider,
    ModelRequest, ModelStream, StopReason, Usage,
};
use tokio::sync::Notify;

#[derive(Clone, Debug, Default)]
pub struct DeterministicTestProvider {
    responses: Arc<Mutex<VecDeque<Result<AssistantMessage, ModelError>>>>,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl DeterministicTestProvider {
    #[allow(dead_code)]
    pub fn new(text: impl Into<String>) -> Self {
        Self::from_texts([text.into()])
    }

    #[allow(dead_code)]
    pub fn from_results<I>(responses: I) -> Self
    where
        I: IntoIterator<Item = Result<AssistantMessage, ModelError>>,
    {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn from_texts<I>(texts: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        Self::from_results(texts.into_iter().map(|text| Ok(text_message(text))))
    }

    #[allow(dead_code)]
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .expect("request log mutex should not be poisoned")
            .clone()
    }
}

impl ModelProvider for DeterministicTestProvider {
    fn stream(&self, req: ModelRequest) -> ModelStream {
        self.requests
            .lock()
            .expect("request log mutex should not be poisoned")
            .push(req);

        let response = self
            .responses
            .lock()
            .expect("response queue mutex should not be poisoned")
            .pop_front()
            .unwrap_or_else(|| {
                Err(ModelError::Protocol(
                    "no deterministic test response queued".to_owned(),
                ))
            });

        let (stream, writer) = channel(NonZeroUsize::new(8).expect("non-zero capacity"));

        tokio::spawn(async move {
            match response {
                Ok(message) => {
                    let text = message
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let _ = writer.emit(ModelEvent::Start).await;
                    let _ = writer
                        .emit(ModelEvent::TextStart { content_index: 0 })
                        .await;
                    let _ = writer
                        .emit(ModelEvent::TextDelta {
                            content_index: 0,
                            delta: text,
                        })
                        .await;
                    let _ = writer.emit(ModelEvent::TextEnd { content_index: 0 }).await;
                    let _ = writer
                        .emit(ModelEvent::Done {
                            reason: done_reason(message.stop),
                            message: message.clone(),
                        })
                        .await;
                    let _ = writer.finish(Ok(message));
                }
                Err(error) => {
                    let _ = writer
                        .emit(ModelEvent::Error {
                            reason: operator_agent::model::ErrorReason::Error,
                            error: error.clone(),
                        })
                        .await;
                    let _ = writer.finish(Err(error));
                }
            }
        });

        stream
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct BlockingTestProvider {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
    started: Arc<Notify>,
    release: Arc<Notify>,
}

impl BlockingTestProvider {
    #[allow(dead_code)]
    pub fn new(started: Arc<Notify>, release: Arc<Notify>) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            started,
            release,
        }
    }

    #[allow(dead_code)]
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .expect("request log mutex should not be poisoned")
            .clone()
    }
}

impl ModelProvider for BlockingTestProvider {
    fn stream(&self, req: ModelRequest) -> ModelStream {
        self.requests
            .lock()
            .expect("request log mutex should not be poisoned")
            .push(req);

        let started = self.started.clone();
        let release = self.release.clone();
        let (stream, writer) = channel(NonZeroUsize::new(8).expect("non-zero capacity"));

        tokio::spawn(async move {
            started.notify_one();
            release.notified().await;
            let _ = writer.finish(Err(ModelError::Protocol(
                "blocking provider released without a deterministic response".into(),
            )));
        });

        stream
    }
}

fn text_message(text: String) -> AssistantMessage {
    AssistantMessage {
        content: vec![ContentBlock::Text { text }],
        usage: Usage::default(),
        stop: StopReason::Stop,
        error_message: None,
        timestamp_ms: 0,
    }
}

fn done_reason(stop: StopReason) -> DoneReason {
    match stop {
        StopReason::Stop => DoneReason::Stop,
        StopReason::Length => DoneReason::Length,
        StopReason::ToolUse => DoneReason::ToolUse,
        StopReason::Aborted | StopReason::Error => DoneReason::Stop,
    }
}
