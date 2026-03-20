use async_trait::async_trait;
use operator_core::{OperatorError, SessionId};

use crate::{Session, SessionEvent, SessionStore};

pub struct NullSessionStore;

#[async_trait]
impl SessionStore for NullSessionStore {
    async fn create(&self, _: &Session) -> Result<(), OperatorError> {
        Ok(())
    }

    async fn append(&self, _: &SessionId, _: &SessionEvent) -> Result<(), OperatorError> {
        Ok(())
    }

    async fn get(&self, _: &SessionId) -> Result<Option<Session>, OperatorError> {
        Ok(None)
    }

    async fn list(&self, _: Option<usize>) -> Result<Vec<SessionId>, OperatorError> {
        Ok(vec![])
    }
}
