use std::{
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use operator_core::{Capability, SessionId, Snapshot};
use operator_runtime::{Runtime, Session, SessionEvent, SessionStatus};
use serde_json::{json, Value};
use tokio::fs;

use crate::{
    journal::SessionJournal,
    model::{
        AssistantMessage, ContentBlock, Message, ModelError, ModelRegistry, ModelRequest,
        ResolvedModel, UserMessage,
    },
    planner::{
        AgentDecision, DecisionParser, DecisionValidator, FinishGate, FinishGateVerdict,
        LoopStateContextManager, PlannerPromptBuilder, PlannerVisualInput,
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
    finish_gate: FinishGate,
}

impl AgentRunner {
    pub fn new(runtime: Arc<Runtime>, models: ModelRegistry, config: AgentConfig) -> Self {
        Self {
            runtime,
            models,
            config,
            prompt_builder: PlannerPromptBuilder::new(),
            parser: DecisionParser::new(),
            finish_gate: FinishGate::new(),
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
        let mut journal = SessionJournal::new(state.session_id.clone());

        let result = self
            .run_loop(&model_name, &model, &mut state, &mut journal)
            .await;

        self.flush_session_journal(&mut journal).await?;
        result
    }

    async fn run_loop(
        &self,
        model_name: &str,
        model: &ResolvedModel,
        state: &mut AgentSessionState,
        journal: &mut SessionJournal,
    ) -> Result<AgentRunResult, AgentError> {
        self.record_user_input(journal, state).await?;

        let executor = ToolExecutor::new(self.runtime.core(), self.runtime.tools().clone());
        let tools = executor.catalog(&state.target)?;
        let validator = DecisionValidator::new(&tools);
        let planner_retry = PlannerRetryPolicy::new(self.config.max_parse_attempts);
        let repeated_error = RepeatedErrorPolicy::new(self.config.repeated_error_limit);

        if let Some(reason) = self
            .maybe_auto_observe(&executor, journal, state, None)
            .await?
        {
            return self.fail_run(journal, state, reason).await;
        }

        for _ in 0..self.config.max_steps {
            state.start_turn();
            state.start_step();

            let decision = self
                .next_decision(model, &tools, &validator, &planner_retry, journal, state)
                .await?;

            match decision {
                AgentDecision::CallTool {
                    name, arguments, ..
                } => {
                    let result = self
                        .execute_tool(&executor, journal, state, &name, arguments)
                        .await?;

                    if let RepeatedErrorDecision::Stop { reason, .. } =
                        repeated_error.register_tool_result(state, &result)
                    {
                        return self.fail_run(journal, state, reason).await;
                    }

                    if should_auto_observe_after_tool(&result) {
                        if let Some(reason) = self
                            .maybe_auto_observe(&executor, journal, state, Some(name.as_str()))
                            .await?
                        {
                            return self.fail_run(journal, state, reason).await;
                        }
                    }
                }
                AgentDecision::Finish { summary } => {
                    let verdict = self
                        .finish_gate
                        .evaluate(model, state, &summary)
                        .await
                        .map_err(|error| self.decorate_model_failure("finish_gate", error))?;

                    match verdict {
                        FinishGateVerdict::Ok { .. } => {
                            state.complete(summary.clone());
                            self.append_session_event(
                                journal,
                                SessionEvent::Completed {
                                    summary: Some(summary.clone()),
                                },
                            )
                            .await?;
                            self.flush_session_journal(journal).await?;

                            return Ok(AgentRunResult {
                                session_id: state.session_id.clone(),
                                target: state.target.clone(),
                                model: model_name.to_string(),
                                summary,
                            });
                        }
                        FinishGateVerdict::NotOk { .. } => {
                            self.finish_gate.record_feedback(state, &verdict);
                        }
                    }
                }
                AgentDecision::Fail { reason } => {
                    return self.fail_run(journal, state, reason).await;
                }
            }

            self.flush_session_journal(journal).await?;
        }

        self.fail_run(
            journal,
            state,
            format!(
                "agent stopped after reaching max_steps ({})",
                self.config.max_steps
            ),
        )
        .await
    }

    async fn maybe_auto_observe(
        &self,
        executor: &ToolExecutor,
        journal: &mut SessionJournal,
        state: &mut AgentSessionState,
        trigger_tool: Option<&str>,
    ) -> Result<Option<String>, AgentError> {
        if !self.supports_auto_observe(&state.target)? {
            return Ok(None);
        }

        let result = self
            .execute_tool(
                executor,
                journal,
                state,
                "observe",
                default_auto_observe_arguments(),
            )
            .await?;
        if result.is_error {
            return Ok(Some(auto_observe_failure_reason(trigger_tool, &result)));
        }

        Ok(None)
    }

    async fn next_decision(
        &self,
        model: &ResolvedModel,
        tools: &[AgentToolSpec],
        validator: &DecisionValidator,
        retry_policy: &PlannerRetryPolicy,
        journal: &mut SessionJournal,
        state: &mut AgentSessionState,
    ) -> Result<AgentDecision, AgentError> {
        loop {
            let planner_context =
                LoopStateContextManager::new(self.runtime.core()).assemble(state)?;
            let visual_inputs = self.load_planner_visuals(&planner_context).await?;
            let prompt = self.prompt_builder.assemble(
                &state.task,
                &planner_context,
                tools,
                state.model_context(),
                &visual_inputs,
            );
            let assistant = self
                .call_model(model, prompt)
                .await
                .map_err(|error| self.decorate_model_failure("planner", error))?;
            let raw = assistant_text(&assistant)?;

            self.append_model_response(journal, state, &assistant, &raw)
                .await?;

            let decision = match self.parser.parse(&raw) {
                Ok(decision) => decision,
                Err(error) => {
                    match retry_policy.register_failure(state, PlannerFailureStage::Parse, &error) {
                        PlannerRetryDecision::Retry { .. } => continue,
                        PlannerRetryDecision::Stop { reason, .. } => {
                            return self.fail_run(journal, state, reason).await;
                        }
                    }
                }
            };

            if let Err(error) = validator.validate(&decision) {
                match retry_policy.register_failure(state, PlannerFailureStage::Validation, &error)
                {
                    PlannerRetryDecision::Retry { .. } => continue,
                    PlannerRetryDecision::Stop { reason, .. } => {
                        return self.fail_run(journal, state, reason).await;
                    }
                }
            }

            return Ok(decision);
        }
    }

    async fn execute_tool(
        &self,
        executor: &ToolExecutor,
        journal: &mut SessionJournal,
        state: &mut AgentSessionState,
        name: &str,
        arguments: Value,
    ) -> Result<AgentToolResult, AgentError> {
        self.append_session_event(
            journal,
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
            journal,
            SessionEvent::ToolResult {
                name: name.to_string(),
                output: payload.clone(),
            },
        )
        .await?;

        let timestamp_ms = now_ms();
        state.push_tool_trace(result.clone(), timestamp_ms);
        state.push_tool_result_message(tool_call_id(state, name), &result, timestamp_ms);

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
        journal: &mut SessionJournal,
        state: &mut AgentSessionState,
        assistant: &AssistantMessage,
        raw: &str,
    ) -> Result<(), AgentError> {
        self.append_session_event(
            journal,
            SessionEvent::ModelResponse {
                content: raw.to_string(),
            },
        )
        .await?;
        state.push_message(Message::Assistant(assistant.clone()));
        Ok(())
    }

    async fn record_user_input(
        &self,
        journal: &mut SessionJournal,
        state: &mut AgentSessionState,
    ) -> Result<(), AgentError> {
        self.append_session_event(
            journal,
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
        journal: &mut SessionJournal,
        event: SessionEvent,
    ) -> Result<(), AgentError> {
        journal.record(event);
        Ok(())
    }

    async fn fail_run<T>(
        &self,
        journal: &mut SessionJournal,
        state: &mut AgentSessionState,
        reason: String,
    ) -> Result<T, AgentError> {
        state.fail(reason.clone());
        self.append_session_event(
            journal,
            SessionEvent::Error {
                message: reason.clone(),
            },
        )
        .await?;
        self.flush_session_journal(journal).await?;
        Err(AgentError::Planner(reason))
    }

    async fn flush_session_journal(&self, journal: &mut SessionJournal) -> Result<(), AgentError> {
        if journal.is_empty() {
            return Ok(());
        }

        let session_store = self.runtime.core().sessions();
        journal.flush(session_store.as_ref()).await?;
        Ok(())
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

    fn supports_auto_observe(&self, target: &operator_core::TargetId) -> Result<bool, AgentError> {
        let (_, driver) = self.runtime.core().resolve_driver(target)?;
        Ok(driver.capabilities().supports(&Capability::Capture))
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

    async fn load_planner_visuals(
        &self,
        planner_context: &crate::planner::PlannerContext,
    ) -> Result<Vec<PlannerVisualInput>, AgentError> {
        let artifact_store = self.runtime.core().artifacts();
        let mut visuals = Vec::new();

        for reference in planner_context.visual_references() {
            let path = artifact_store
                .resolve_artifact(&reference.artifact_id)
                .await?;
            let bytes = match fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(operator_core::OperatorError::from(error).into()),
            };
            let Some(mime) = screenshot_mime(&path) else {
                continue;
            };

            visuals.push(PlannerVisualInput {
                slot: reference.slot,
                image: ContentBlock::Image {
                    mime: mime.into(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                },
            });
        }

        Ok(visuals)
    }
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

fn should_auto_observe_after_tool(result: &AgentToolResult) -> bool {
    !result.is_error && !result.read_only
}

fn default_auto_observe_arguments() -> Value {
    json!({
        "surface": { "kind": "Frontmost" },
        "include_screenshot": true,
        "include_elements": false,
    })
}

fn auto_observe_failure_reason(trigger_tool: Option<&str>, result: &AgentToolResult) -> String {
    let detail = result
        .error
        .as_ref()
        .map(|error| error.message.clone())
        .unwrap_or_else(|| "unknown observe failure".into());

    match trigger_tool {
        Some(tool_name) => {
            format!("automatic screenshot observe after `{tool_name}` failed: {detail}")
        }
        None => format!("automatic screenshot observe before planning failed: {detail}"),
    }
}

fn screenshot_mime(path: &Path) -> Option<&'static str> {
    let extension = path.extension().and_then(|extension| extension.to_str())?;

    match extension.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}
