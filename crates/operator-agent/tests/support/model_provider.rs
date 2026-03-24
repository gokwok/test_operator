use std::{num::NonZeroUsize, sync::Arc};

use operator_agent::model::{
    channel, AssistantMessage, ContentBlock, DoneReason, ModelEvent, ModelProvider, ModelRequest,
    ModelStream, StopReason, Usage,
};

#[derive(Clone, Debug)]
pub struct DeterministicTestProvider {
    text: Arc<str>,
}

impl DeterministicTestProvider {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self { text: text.into() }
    }
}

impl ModelProvider for DeterministicTestProvider {
    fn stream(&self, _req: ModelRequest) -> ModelStream {
        let (stream, writer) = channel(NonZeroUsize::new(8).expect("non-zero capacity"));
        let text = self.text.clone();

        tokio::spawn(async move {
            let message = AssistantMessage {
                content: vec![ContentBlock::Text {
                    text: text.to_string(),
                }],
                usage: Usage::default(),
                stop: StopReason::Stop,
                error_message: None,
                timestamp_ms: 0,
            };

            let _ = writer.emit(ModelEvent::Start).await;
            let _ = writer
                .emit(ModelEvent::TextStart { content_index: 0 })
                .await;
            let _ = writer
                .emit(ModelEvent::TextDelta {
                    content_index: 0,
                    delta: text.to_string(),
                })
                .await;
            let _ = writer.emit(ModelEvent::TextEnd { content_index: 0 }).await;
            let _ = writer
                .emit(ModelEvent::Done {
                    reason: DoneReason::Stop,
                    message: message.clone(),
                })
                .await;
            let _ = writer.finish(Ok(message));
        });

        stream
    }
}
