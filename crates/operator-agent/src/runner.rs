use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use operator_core::{SessionId, Snapshot};
use operator_runtime::{Runtime, Session, SessionEvent, SessionStatus};
use serde_json::Value;

use crate::{
    model::{
        AssistantMessage, ContentBlock, Message, ModelError, ModelRegistry, ModelRequest,
        ResolvedModel, ToolResultMessage, UserMessage,
    },
    planner::{
        AgentDecision, ContextAssembler, DecisionParser, DecisionValidator, PlannerPromptBuilder,
        TaskReflection, TaskReflector,
    },
    policy::{
        PlannerFailureStage, PlannerRetryDecision, PlannerRetryPolicy, RepeatedErrorDecision,
        RepeatedErrorPolicy,
    },
    session::AgentSessionState,
    tools::{AgentToolResult, AgentToolSpec, ToolExecutor},
    AgentConfig, AgentError, AgentRunRequest, AgentRunResult,
};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct AgentRunner {
    runtime: Arc<Runtime>,
    models: ModelRegistry,
    config: AgentConfig,
    prompt_builder: PlannerPromptBuilder,
    parser: DecisionParser,
    reflector: TaskReflector,
}

impl AgentRunner {
    pub fn new(runtime: Arc<Runtime>, models: ModelRegistry, config: AgentConfig) -> Self {
        Self {
            runtime,
            models,
            config,
            prompt_builder: PlannerPromptBuilder::new(),
            parser: DecisionParser::new(),
            reflector: TaskReflector::new(),
        }
    }

    pub async fn run(&self, req: AgentRunRequest) -> Result<AgentRunResult, AgentError> {
        self.validate_config()?;

        let model_name = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());
        let model = self.resolve_model(&model_name)?;
        let session_id = next_session_id();

        let mut state = AgentSessionState::new(session_id.clone(), req.target.clone(), req.task);
        self.create_runtime_session(&state).await?;
        self.record_user_input(&mut state).await?;

        let executor = ToolExecutor::new(self.runtime.core(), self.runtime.tools().clone());
        let tools = executor.catalog(&state.target)?;
        let validator = DecisionValidator::new(&tools);
        let planner_retry = PlannerRetryPolicy::new(self.config.max_parse_attempts);
        let repeated_error = RepeatedErrorPolicy::new(self.config.repeated_error_limit);

        self.bootstrap_context(&executor, &tools, &mut state)
            .await?;

        for _ in 0..self.config.max_steps {
            state.start_turn();
            state.start_step();

            let decision = self
                .next_decision(&model, &tools, &validator, &planner_retry, &mut state)
                .await?;

            match decision {
                AgentDecision::CallTool {
                    name, arguments, ..
                } => {
                    let result = self
                        .execute_tool(&executor, &mut state, &name, arguments)
                        .await?;

                    if let RepeatedErrorDecision::Stop { reason, .. } =
                        repeated_error.register_tool_result(&mut state, &result)
                    {
                        return self.fail_run(&mut state, reason).await;
                    }
                }
                AgentDecision::Finish { summary } => {
                    let reflection = self
                        .reflector
                        .reflect(&model, &state, &summary)
                        .await
                        .map_err(|error| self.decorate_model_failure("reflector", error))?;

                    match reflection {
                        TaskReflection::Ok { .. } => {
                            state.complete(summary.clone());
                            self.append_session_event(
                                &state.session_id,
                                SessionEvent::Completed {
                                    summary: Some(summary.clone()),
                                },
                            )
                            .await?;

                            return Ok(AgentRunResult {
                                session_id: state.session_id.clone(),
                                target: state.target.clone(),
                                model: model_name,
                                summary,
                            });
                        }
                        TaskReflection::NotOk { .. } => {
                            self.reflector.record_feedback(&mut state, &reflection);
                        }
                    }
                }
                AgentDecision::Fail { reason } => {
                    return self.fail_run(&mut state, reason).await;
                }
            }
        }

        self.fail_run(
            &mut state,
            format!(
                "agent stopped after reaching max_steps ({})",
                self.config.max_steps
            ),
        )
        .await
    }

    async fn bootstrap_context(
        &self,
        executor: &ToolExecutor,
        tools: &[AgentToolSpec],
        state: &mut AgentSessionState,
    ) -> Result<(), AgentError> {
        for name in bootstrap_tool_names(tools) {
            let result = self
                .execute_tool(executor, state, name, Value::Object(Default::default()))
                .await?;
            if result.is_error {
                let message = result
                    .error
                    .as_ref()
                    .map(|error| format!("bootstrap tool `{name}` failed: {}", error.message))
                    .unwrap_or_else(|| format!("bootstrap tool `{name}` failed"));
                return self.fail_run(state, message).await;
            }
        }

        Ok(())
    }

    async fn next_decision(
        &self,
        model: &ResolvedModel,
        tools: &[AgentToolSpec],
        validator: &DecisionValidator,
        retry_policy: &PlannerRetryPolicy,
        state: &mut AgentSessionState,
    ) -> Result<AgentDecision, AgentError> {
        loop {
            let planner_context = ContextAssembler::new(self.runtime.core())
                .assemble(state)
                .await?;
            let prompt =
                self.prompt_builder
                    .assemble(&state.task, &planner_context, tools, &state.messages);
            let assistant = self
                .call_model(model, prompt)
                .await
                .map_err(|error| self.decorate_model_failure("planner", error))?;
            let raw = assistant_text(&assistant)?;

            self.append_model_response(state, &assistant, &raw).await?;

            let decision = match self.parser.parse(&raw) {
                Ok(decision) => decision,
                Err(error) => {
                    match retry_policy.register_failure(state, PlannerFailureStage::Parse, &error) {
                        PlannerRetryDecision::Retry { .. } => continue,
                        PlannerRetryDecision::Stop { reason, .. } => {
                            return self.fail_run(state, reason).await;
                        }
                    }
                }
            };

            if let Err(error) = validator.validate(&decision) {
                match retry_policy.register_failure(state, PlannerFailureStage::Validation, &error)
                {
                    PlannerRetryDecision::Retry { .. } => continue,
                    PlannerRetryDecision::Stop { reason, .. } => {
                        return self.fail_run(state, reason).await;
                    }
                }
            }

            return Ok(decision);
        }
    }

    async fn execute_tool(
        &self,
        executor: &ToolExecutor,
        state: &mut AgentSessionState,
        name: &str,
        arguments: Value,
    ) -> Result<AgentToolResult, AgentError> {
        self.append_session_event(
            &state.session_id,
            SessionEvent::ToolCall {
                name: name.to_string(),
                input: arguments.clone(),
            },
        )
        .await?;

        let result = executor
            .call(
                &state.session_id,
                &state.target,
                name,
                arguments.clone(),
                Some(self.config.step_timeout_ms),
            )
            .await?;

        let payload = serde_json::to_value(&result).expect("agent tool results should serialize");
        self.append_session_event(
            &state.session_id,
            SessionEvent::ToolResult {
                name: name.to_string(),
                output: payload.clone(),
            },
        )
        .await?;

        let timestamp_ms = now_ms();
        state.push_tool_trace(result.clone(), timestamp_ms);
        state.push_message(Message::ToolResult(ToolResultMessage {
            tool_call_id: tool_call_id(state, name),
            tool_name: Arc::<str>::from(name.to_string()),
            content: vec![ContentBlock::Text {
                text: serde_json::to_string_pretty(&payload)
                    .expect("tool result payloads should serialize"),
            }],
            is_error: result.is_error,
            timestamp_ms,
        }));

        if !result.is_error {
            self.update_observation_state(state, &result);
        }

        Ok(result)
    }

    async fn call_model(
        &self,
        model: &ResolvedModel,
        context: crate::model::Context,
    ) -> Result<AssistantMessage, AgentError> {
        let request = ModelRequest {
            config: model.config.clone(),
            context,
            options: model.config.default_options.clone(),
            stream: false,
            timeout: Some(Duration::from_millis(self.config.step_timeout_ms)),
            request_id: Some(Arc::<str>::from(format!("planner-{}", now_ms()))),
            max_retry_delay_ms: None,
        };

        model
            .provider
            .stream(request)
            .result()
            .await
            .map_err(model_error)
    }

    async fn append_model_response(
        &self,
        state: &mut AgentSessionState,
        assistant: &AssistantMessage,
        raw: &str,
    ) -> Result<(), AgentError> {
        self.append_session_event(
            &state.session_id,
            SessionEvent::ModelResponse {
                content: raw.to_string(),
            },
        )
        .await?;
        state.push_message(Message::Assistant(assistant.clone()));
        Ok(())
    }

    async fn record_user_input(&self, state: &mut AgentSessionState) -> Result<(), AgentError> {
        self.append_session_event(
            &state.session_id,
            SessionEvent::UserInput {
                text: state.task.clone(),
            },
        )
        .await?;

        state.push_message(Message::User(UserMessage {
            content: vec![ContentBlock::Text {
                text: state.task.clone(),
            }],
            timestamp_ms: now_ms(),
        }));

        Ok(())
    }

    async fn create_runtime_session(&self, state: &AgentSessionState) -> Result<(), AgentError> {
        self.runtime
            .core()
            .sessions()
            .create(&Session {
                id: state.session_id.clone(),
                created_at: SystemTime::now(),
                task: state.task.clone(),
                status: SessionStatus::Running,
            })
            .await?;
        Ok(())
    }

    async fn append_session_event(
        &self,
        session_id: &SessionId,
        event: SessionEvent,
    ) -> Result<(), AgentError> {
        self.runtime
            .core()
            .sessions()
            .append(session_id, &event)
            .await?;
        Ok(())
    }

    async fn fail_run<T>(
        &self,
        state: &mut AgentSessionState,
        reason: String,
    ) -> Result<T, AgentError> {
        state.fail(reason.clone());
        self.append_session_event(
            &state.session_id,
            SessionEvent::Error {
                message: reason.clone(),
            },
        )
        .await?;
        Err(AgentError::Planner(reason))
    }

    fn resolve_model(&self, name: &str) -> Result<ResolvedModel, AgentError> {
        self.models.resolve(name).map_err(|error| match error {
            ModelError::ModelNotFound(_) | ModelError::ProviderNotFound { .. } => {
                AgentError::ModelNotConfigured(name.to_string())
            }
            other => AgentError::Planner(format!("model resolution failed: {other}")),
        })
    }

    fn decorate_model_failure(&self, stage: &str, error: AgentError) -> AgentError {
        match error {
            AgentError::Planner(message) => {
                AgentError::Planner(format!("{stage} model call failed: {message}"))
            }
            other => other,
        }
    }

    fn update_observation_state(&self, state: &mut AgentSessionState, result: &AgentToolResult) {
        if result.tool_name != "observe" {
            return;
        }

        let Some(snapshot) = result.output.as_ref().and_then(snapshot_from_tool_output) else {
            return;
        };

        state.record_observation_snapshot(&snapshot);
    }

    fn validate_config(&self) -> Result<(), AgentError> {
        if self.config.max_steps == 0 {
            return Err(AgentError::Config(
                "max_steps must be greater than zero".into(),
            ));
        }
        if self.config.step_timeout_ms == 0 {
            return Err(AgentError::Config(
                "step_timeout_ms must be greater than zero".into(),
            ));
        }

        Ok(())
    }
}

fn bootstrap_tool_names(tools: &[AgentToolSpec]) -> Vec<&'static str> {
    let mut names = vec!["capabilities"];
    if tools.iter().any(|tool| tool.name == "permissions-status") {
        names.push("permissions-status");
    }
    if tools.iter().any(|tool| tool.name == "get-focus") {
        names.push("get-focus");
    }
    names
}

fn assistant_text(message: &AssistantMessage) -> Result<String, AgentError> {
    let text = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AgentError::Planner(
            "model response must contain at least one text block".into(),
        ));
    }

    Ok(trimmed.to_string())
}

fn model_error(error: ModelError) -> AgentError {
    match error {
        ModelError::ModelNotFound(name) => AgentError::ModelNotConfigured(name),
        other => AgentError::Planner(other.to_string()),
    }
}

fn next_session_id() -> SessionId {
    SessionId(format!(
        "agent-{}",
        SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn tool_call_id(state: &AgentSessionState, name: &str) -> Arc<str> {
    Arc::<str>::from(format!(
        "tool-{}-{}-{}-{}",
        state.turn_index,
        state.step_index,
        name,
        state.tool_trace.len()
    ))
}

fn snapshot_from_tool_output(output: &Value) -> Option<Snapshot> {
    output
        .get("snapshot")
        .cloned()
        .and_then(|snapshot| serde_json::from_value(snapshot).ok())
}
