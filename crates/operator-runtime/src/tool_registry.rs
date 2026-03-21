use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use operator_core::{ExecContext, OperatorError, SessionId, TargetId};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::RuntimeCore;

type ToolFuture = Pin<Box<dyn Future<Output = Result<Value, OperatorError>> + Send + 'static>>;

pub type ToolHandler =
    Arc<dyn Fn(Value, Arc<RuntimeCore>, ExecContext) -> ToolFuture + Send + Sync + 'static>;

#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
    pub capabilities_required: &'static [operator_core::Capability],
    pub has_side_effects: bool,
}

#[derive(Clone)]
pub struct ToolRegistration {
    pub spec: ToolSpec,
    pub handler: ToolHandler,
}

#[derive(Clone)]
pub struct ToolRegistry {
    core: Arc<RuntimeCore>,
    registrations: HashMap<&'static str, ToolRegistration>,
}

impl ToolRegistry {
    pub fn new(core: Arc<RuntimeCore>) -> Self {
        Self {
            core,
            registrations: HashMap::new(),
        }
    }

    pub fn register(&mut self, registration: ToolRegistration) -> Result<(), OperatorError> {
        let name = registration.spec.name;
        if self.registrations.contains_key(name) {
            return Err(OperatorError::Tool {
                tool: name.to_string(),
                message: "tool already registered".into(),
            });
        }

        self.registrations.insert(name, registration);
        Ok(())
    }

    pub fn register_all(
        &mut self,
        registrations: Vec<ToolRegistration>,
    ) -> Result<(), OperatorError> {
        for registration in registrations {
            self.register(registration)?;
        }

        Ok(())
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut specs = self
            .registrations
            .values()
            .map(|registration| registration.spec.clone())
            .collect::<Vec<_>>();
        specs.sort_by_key(|spec| spec.name);
        specs
    }

    pub async fn invoke(&self, name: &str, input: Value) -> Result<Value, OperatorError> {
        let registration = self
            .registrations
            .get(name)
            .ok_or_else(|| OperatorError::Tool {
                tool: name.to_string(),
                message: "tool is not registered".into(),
            })?;

        let ctx = self.extract_exec_context(&input)?;
        (registration.handler)(input, self.core.clone(), ctx).await
    }

    fn extract_exec_context(&self, input: &Value) -> Result<ExecContext, OperatorError> {
        let parsed: ExecContextInput =
            serde_json::from_value(input.clone()).map_err(|error| OperatorError::Tool {
                tool: "tool-registry".into(),
                message: format!("invalid exec context: {error}"),
            })?;

        Ok(ExecContext {
            target: parsed
                .target
                .unwrap_or_else(|| self.core.config().default_target.clone()),
            session: parsed.session_id,
            timeout_ms: Some(
                parsed
                    .timeout_ms
                    .unwrap_or(self.core.config().default_timeout_ms),
            ),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ExecContextInput {
    target: Option<TargetId>,
    session_id: Option<SessionId>,
    timeout_ms: Option<u64>,
}
