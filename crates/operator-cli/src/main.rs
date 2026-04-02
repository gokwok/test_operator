#![cfg_attr(test, allow(dead_code))]

pub(crate) mod args;
mod output;

use std::{
    collections::BTreeSet,
    future::Future,
    io::{self, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use indicatif::{ProgressBar, ProgressStyle};

use operator_agent::{
    model::{ModelRegistry, SelectedModelProviderConfig},
    AgentConfig, AgentProgressEvent, AgentProgressReporter, AgentRunRequest, AgentRunResult,
    AgentRunner,
};
use operator_bootstrap::{
    default_model_api_kind_for_selector, load_bootstrap_config_from, load_runtime_config,
    operator_home_dir, parse_model_set_expression, parse_target_set_expression,
    runtime_config_path, system_platform_registry, AgentModelConfig, ModelConfigFieldPath,
    RuntimeConfigDocument, TargetConfigFieldPath,
};
use operator_core::{OperatorError, TargetId};
#[cfg(not(test))]
use operator_mcp::run_stdio_server;
use operator_runtime::{
    FileArtifactStore, FileSessionStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig,
    ToolRegistry,
};
use serde_json::Value;

use self::args::{AgentCommand, Cli, CliExecution, ModelCommand, TargetCommand, ToolInvocation};

type InvokeFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, OperatorError>> + Send + 'a>>;
type AgentFuture<'a> = Pin<Box<dyn Future<Output = Result<AgentRunResult, String>> + Send + 'a>>;
type InspectFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, OperatorError>> + Send + 'a>>;

pub(crate) trait ToolInvoker {
    fn invoke<'a>(&'a self, tool: &'a str, input: Value) -> InvokeFuture<'a>;
}

pub(crate) trait AgentExecutor {
    fn run<'a>(&'a self, command: &'a AgentCommand) -> AgentFuture<'a>;
}

pub(crate) trait TargetInspector {
    fn inspect<'a>(&'a self, command: &'a TargetCommand) -> InspectFuture<'a>;
}

pub(crate) trait ModelInspector {
    fn inspect<'a>(&'a self, command: &'a ModelCommand) -> InspectFuture<'a>;
}

struct RuntimeToolInvoker {
    tools: ToolRegistry,
}

impl RuntimeToolInvoker {
    async fn build() -> Result<Self, OperatorError> {
        let runtime = build_runtime(load_runtime_config()?).await?;

        Ok(Self {
            tools: runtime.tools().clone(),
        })
    }
}

impl ToolInvoker for RuntimeToolInvoker {
    fn invoke<'a>(&'a self, tool: &'a str, input: Value) -> InvokeFuture<'a> {
        Box::pin(async move { self.tools.invoke(tool, input).await })
    }
}

struct RuntimeAgentExecutor;

impl AgentExecutor for RuntimeAgentExecutor {
    fn run<'a>(&'a self, command: &'a AgentCommand) -> AgentFuture<'a> {
        Box::pin(async move {
            let prepared = agent_execution_for_home(command, operator_home_dir())
                .map_err(|error| error.to_string())?;
            let request = AgentRunRequest {
                task: command.task.clone(),
                target: prepared.runtime_config.default_target.clone(),
                model: command.model.clone(),
                app: command.app.clone(),
            };
            let runtime = build_runtime(prepared.runtime_config)
                .await
                .map_err(|error| error.to_string())?;
            let mut runner =
                AgentRunner::new(Arc::new(runtime), prepared.models, prepared.agent_config);
            if !command.json_output {
                runner =
                    runner.with_progress_reporter(Arc::new(ConsoleAgentProgressReporter::new()));
            }
            runner.run(request).await.map_err(|error| error.to_string())
        })
    }
}

struct ConsoleAgentProgressReporter {
    renderer: Mutex<output::AgentProgressRenderer>,
    active_spinner: Mutex<Option<ProgressBar>>,
    /// Thinking line buffered from PlannedTool / FinishPlanned.
    /// Displayed as the spinner message while the tool runs, then printed
    /// statically once the result arrives.
    pending_thinking: Mutex<Option<String>>,
}

impl ConsoleAgentProgressReporter {
    fn new() -> Self {
        Self {
            renderer: Mutex::new(output::AgentProgressRenderer::new()),
            active_spinner: Mutex::new(None),
            pending_thinking: Mutex::new(None),
        }
    }

    /// Print a line, routing through the active spinner so it doesn't get garbled.
    fn print_line(&self, line: &str) {
        let guard = self.active_spinner.lock().expect("spinner mutex poisoned");
        if let Some(pb) = guard.as_ref() {
            pb.println(line);
        } else {
            let _ = writeln!(io::stderr().lock(), "{line}");
        }
    }

    /// Stop and erase any running spinner.
    fn clear_spinner(&self) {
        if let Some(pb) = self
            .active_spinner
            .lock()
            .expect("spinner mutex poisoned")
            .take()
        {
            pb.finish_and_clear();
        }
    }

    /// Print and consume any buffered thinking line.
    fn flush_thinking(&self) {
        if let Some(line) = self
            .pending_thinking
            .lock()
            .expect("thinking mutex poisoned")
            .take()
        {
            self.print_line(&line);
        }
    }
}

impl AgentProgressReporter for ConsoleAgentProgressReporter {
    fn report(&self, event: AgentProgressEvent) {
        // TurnStarted: clear any prior spinner, flush buffered thinking, then
        // start a fresh spinner immediately so the user sees activity during
        // LLM inference (before PlannedTool arrives).
        if let AgentProgressEvent::TurnStarted { turn_index: _ } = &event {
            self.clear_spinner();
            self.flush_thinking();
            // Print any blank-line separator the renderer emits.
            let rendered = {
                let mut renderer = self.renderer.lock().expect("renderer mutex poisoned");
                renderer.render(&event)
            };
            if let Some(ref text) = rendered {
                if !text.is_empty() {
                    self.print_line(text);
                }
            }
            // Start spinner immediately with no message —
            // PlannedTool will fill it in once the model decides what to do.
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("  {spinner:.cyan}")
                    .expect("spinner template is valid"),
            );
            pb.enable_steady_tick(Duration::from_millis(80));
            *self.active_spinner.lock().expect("spinner mutex poisoned") = Some(pb);
            return;
        }

        // PlannedTool / FinishPlanned: update running spinner message in-place,
        // and buffer the full thinking line for later static printing.
        if matches!(
            &event,
            AgentProgressEvent::PlannedTool { .. } | AgentProgressEvent::FinishPlanned { .. }
        ) {
            let rendered = {
                let mut renderer = self.renderer.lock().expect("renderer mutex poisoned");
                renderer.render(&event)
            };
            if let Some(ref text) = rendered {
                if let Some(pb) = self
                    .active_spinner
                    .lock()
                    .expect("spinner mutex poisoned")
                    .as_ref()
                {
                    pb.set_message(text.trim_start().to_string());
                }
            }
            *self
                .pending_thinking
                .lock()
                .expect("thinking mutex poisoned") = rendered;
            return;
        }

        // ToolCall: reuse the running spinner (regular turn) so the thinking
        // line continues animating during tool execution.  For setup turns that
        // never got a TurnStarted, start a fresh spinner.
        if let AgentProgressEvent::ToolCall { name, args, .. } = &event {
            let rendered = {
                let mut renderer = self.renderer.lock().expect("renderer mutex poisoned");
                renderer.render(&event)
            };

            // Print any section header preamble (e.g. "  setup") that precedes
            // the tool line.
            if let Some(ref text) = rendered {
                let all: Vec<&str> = text.lines().collect();
                if all.len() > 1 {
                    self.print_line(&all[..all.len() - 1].join("\n"));
                }
            }

            // Spinner message = thinking line (trimmed of leading indent).
            let thinking = self
                .pending_thinking
                .lock()
                .expect("thinking mutex poisoned")
                .clone();
            let spinner_msg = match thinking {
                Some(ref t) => t.trim_start().to_string(),
                None => output::tool_call_label(name, args),
            };

            let spinner_running = self
                .active_spinner
                .lock()
                .expect("spinner mutex poisoned")
                .is_some();
            if spinner_running {
                // Update the existing spinner message in-place.
                if let Some(pb) = self
                    .active_spinner
                    .lock()
                    .expect("spinner mutex poisoned")
                    .as_ref()
                {
                    pb.set_message(spinner_msg);
                }
            } else {
                // Setup turn: start a fresh spinner.
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::with_template("  {spinner:.cyan} {msg}")
                        .expect("spinner template is valid"),
                );
                pb.set_message(spinner_msg);
                pb.enable_steady_tick(Duration::from_millis(80));
                *self.active_spinner.lock().expect("spinner mutex poisoned") = Some(pb);
            }
            return;
        }

        // ToolResult: stop spinner, print thinking line statically, then result.
        if matches!(&event, AgentProgressEvent::ToolResult { .. }) {
            self.clear_spinner();
            self.flush_thinking();
            let rendered = {
                let mut renderer = self.renderer.lock().expect("renderer mutex poisoned");
                renderer.render(&event)
            };
            if let Some(text) = rendered {
                self.print_line(&text);
            }
            return;
        }

        // All other events: stop spinner, flush any pending thinking, then print.
        self.clear_spinner();
        self.flush_thinking();
        let rendered = {
            let mut renderer = self.renderer.lock().expect("renderer mutex poisoned");
            renderer.render(&event)
        };
        if let Some(text) = rendered {
            self.print_line(&text);
        }
    }
}

pub(crate) struct AgentExecutionBootstrap {
    pub(crate) runtime_config: RuntimeConfig,
    pub(crate) agent_config: AgentConfig,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) selected_model: String,
    pub(crate) models: ModelRegistry,
}

struct RuntimeConfigInspector {
    operator_home: PathBuf,
}

impl RuntimeConfigInspector {
    fn new(operator_home: PathBuf) -> Self {
        Self { operator_home }
    }
}

impl TargetInspector for RuntimeConfigInspector {
    fn inspect<'a>(&'a self, command: &'a TargetCommand) -> InspectFuture<'a> {
        Box::pin(async move { inspect_target_command(command, &self.operator_home) })
    }
}

impl ModelInspector for RuntimeConfigInspector {
    fn inspect<'a>(&'a self, command: &'a ModelCommand) -> InspectFuture<'a> {
        Box::pin(async move { inspect_model_command(command, &self.operator_home) })
    }
}

#[cfg(not(test))]
fn main() {
    std::process::exit(process_entry());
}

#[cfg(not(test))]
fn process_entry() -> i32 {
    #[cfg(feature = "macos-action-effects")]
    match operator_platform_macos::try_run_action_effect_helper() {
        Ok(Some(code)) => return code,
        Ok(None) => {}
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    }

    tokio_main()
}

#[cfg(not(test))]
#[tokio::main]
async fn tokio_main() -> i32 {
    main_entry().await
}

#[cfg(not(test))]
async fn main_entry() -> i32 {
    let cli = Cli::parse();
    let json_output = cli.prefers_json();

    let execution = match cli.into_execution() {
        Ok(execution) => execution,
        Err(error) => {
            eprintln!("{}", output::render_error(json_output, &error));
            return 1;
        }
    };

    match execution {
        CliExecution::Tool(invocation) => {
            let invoker = match RuntimeToolInvoker::build().await {
                Ok(invoker) => invoker,
                Err(error) => {
                    eprintln!("{}", output::render_error(json_output, &error.to_string()));
                    return 1;
                }
            };

            match run_invocation_with_invoker(invocation, &invoker).await {
                Ok(rendered) => {
                    println!("{rendered}");
                    0
                }
                Err(error) => {
                    eprintln!(
                        "{}",
                        output::render_error(json_output, &format_operator_error(&error))
                    );
                    1
                }
            }
        }
        CliExecution::Target(command) => {
            let inspector = RuntimeConfigInspector::new(operator_home_dir());
            match run_target_with_inspector(command, &inspector).await {
                Ok(rendered) => {
                    println!("{rendered}");
                    0
                }
                Err(error) => {
                    eprintln!(
                        "{}",
                        output::render_error(json_output, &format_operator_error(&error))
                    );
                    1
                }
            }
        }
        CliExecution::Model(command) => {
            let inspector = RuntimeConfigInspector::new(operator_home_dir());
            match run_model_with_inspector(command, &inspector).await {
                Ok(rendered) => {
                    println!("{rendered}");
                    0
                }
                Err(error) => {
                    eprintln!(
                        "{}",
                        output::render_error(json_output, &format_operator_error(&error))
                    );
                    1
                }
            }
        }
        CliExecution::Agent(_command) => {
            let executor = RuntimeAgentExecutor;
            match run_agent_with_executor(_command, &executor).await {
                Ok(rendered) => {
                    println!("{rendered}");
                    0
                }
                Err(error) => {
                    if should_render_agent_error(json_output, &error) {
                        eprintln!("{}", output::render_error(json_output, &error));
                    }
                    1
                }
            }
        }
        CliExecution::McpServe => match run_stdio_server().await {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        },
    }
}

async fn run_invocation_with_invoker(
    invocation: ToolInvocation,
    invoker: &impl ToolInvoker,
) -> Result<String, OperatorError> {
    let output = invoker.invoke(invocation.tool, invocation.input).await?;

    Ok(output::render_success(
        invocation.tool,
        &output,
        invocation.json_output,
    ))
}

async fn run_agent_with_executor(
    command: AgentCommand,
    executor: &impl AgentExecutor,
) -> Result<String, String> {
    let json_output = command.json_output;
    let result = executor.run(&command).await?;
    Ok(output::render_agent_success(&result, json_output))
}

pub(crate) fn should_render_agent_error(json_output: bool, error: &str) -> bool {
    if json_output {
        return true;
    }

    !error.trim_start().starts_with("agent run interrupted:")
}

async fn run_target_with_inspector(
    command: TargetCommand,
    inspector: &impl TargetInspector,
) -> Result<String, OperatorError> {
    let (tool, json_output) = match &command {
        TargetCommand::List { json_output } => ("target-list", *json_output),
        TargetCommand::Show { json_output, .. } => ("target-show", *json_output),
        TargetCommand::Use { json_output, .. } => ("target-use", *json_output),
        TargetCommand::Set { json_output, .. } => ("target-set", *json_output),
        TargetCommand::Unset { json_output, .. } => ("target-unset", *json_output),
        TargetCommand::Remove { json_output, .. } => ("target-remove", *json_output),
    };
    let output = inspector.inspect(&command).await?;
    Ok(output::render_success(tool, &output, json_output))
}

async fn run_model_with_inspector(
    command: ModelCommand,
    inspector: &impl ModelInspector,
) -> Result<String, OperatorError> {
    let (tool, json_output) = match &command {
        ModelCommand::List { json_output } => ("model-list", *json_output),
        ModelCommand::Show { json_output, .. } => ("model-show", *json_output),
        ModelCommand::Use { json_output, .. } => ("model-use", *json_output),
        ModelCommand::Set { json_output, .. } => ("model-set", *json_output),
        ModelCommand::Unset { json_output, .. } => ("model-unset", *json_output),
    };
    let output = inspector.inspect(&command).await?;
    Ok(output::render_success(tool, &output, json_output))
}

async fn build_runtime(config: RuntimeConfig) -> Result<operator_runtime::Runtime, OperatorError> {
    let root = operator_home_dir();
    let snapshots = Arc::new(FileSnapshotStore::new(&root, config.clone()));
    let artifacts = Arc::new(FileArtifactStore::new(&root));
    let sessions = Arc::new(FileSessionStore::new(&root));

    RuntimeBuilder::new(config)
        .artifact_store(artifacts.clone())
        .snapshot_store(snapshots)
        .session_store(sessions)
        .platform_registry(system_platform_registry(artifacts.artifacts_dir()))
        .build()
        .await
}

pub(crate) fn agent_execution_for_home(
    command: &AgentCommand,
    operator_home: impl AsRef<Path>,
) -> Result<AgentExecutionBootstrap, String> {
    let bootstrap =
        load_bootstrap_config_from(&operator_home).map_err(|error| error.to_string())?;
    let mut runtime_config = bootstrap.runtime;
    if let Some(target) = &command.target {
        runtime_config.default_target = target.clone().into();
    }
    if let Some(timeout_ms) = command.timeout_ms {
        runtime_config.default_timeout_ms = timeout_ms;
    }

    let agent_config = agent_config_for(command, bootstrap.agent_model.default.as_deref());
    let selected_model = command
        .model
        .clone()
        .unwrap_or_else(|| agent_config.default_model.clone());
    let provider_config = configured_model_provider(&bootstrap.agent_model, &selected_model);
    let models = ModelRegistry::from_selected_provider_config(&selected_model, &provider_config)
        .map_err(|error| error.to_string())?;

    Ok(AgentExecutionBootstrap {
        runtime_config,
        agent_config,
        selected_model,
        models,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_config_for_home(
    command: &AgentCommand,
    operator_home: impl AsRef<Path>,
) -> Result<RuntimeConfig, OperatorError> {
    let mut config = operator_bootstrap::load_runtime_config_from(operator_home)?;
    if let Some(target) = &command.target {
        config.default_target = target.clone().into();
    }
    if let Some(timeout_ms) = command.timeout_ms {
        config.default_timeout_ms = timeout_ms;
    }
    Ok(config)
}

fn configured_model_provider(
    agent_model: &AgentModelConfig,
    selector: &str,
) -> SelectedModelProviderConfig {
    agent_model
        .providers
        .get(selector)
        .map(|provider| SelectedModelProviderConfig {
            api_key: provider.api_key.clone(),
            base_url: provider.base_url.clone(),
            model_name: provider.model_name.clone(),
            api_kind: provider.api_kind.clone(),
        })
        .unwrap_or_default()
}

fn agent_config_for(command: &AgentCommand, default_model: Option<&str>) -> AgentConfig {
    let mut config = AgentConfig::default();
    if let Some(default_model) = default_model {
        config.default_model = default_model.to_owned();
    }
    config.include_elements = command.include_elements;
    if let Some(max_steps) = command.max_steps {
        config.max_steps = max_steps.get();
    }
    if let Some(timeout_ms) = command.timeout_ms {
        config.step_timeout_ms = timeout_ms;
    }
    if let Some(observe_delay_ms) = command.observe_delay_ms {
        config.observe_delay_ms = observe_delay_ms;
    }
    config
}

pub(crate) fn inspect_target_command(
    command: &TargetCommand,
    operator_home: impl AsRef<Path>,
) -> Result<Value, OperatorError> {
    let path = runtime_config_path(operator_home);
    let mut document = RuntimeConfigDocument::load(&path)?;
    let config = document.to_runtime_config()?;
    let persisted_target_names = document.persisted_target_names();

    match command {
        TargetCommand::List { .. } => Ok(serde_json::json!({
            "default_target": config.default_target.to_string(),
            "targets": persisted_target_names.iter().filter_map(|name| {
                config.targets.get(name).map(|target| serde_json::json!({
                    "name": name,
                    "is_default": config.default_target == TargetId(name.clone()),
                    "platform": target.platform,
                    "driver": target.driver,
                    "description": target.description,
                }))
            }).collect::<Vec<_>>(),
        })),
        TargetCommand::Show { name, .. } => {
            let selected = name
                .clone()
                .unwrap_or_else(|| config.default_target.to_string());
            if !document.has_persisted_named_target(&selected) {
                return Err(OperatorError::TargetNotFound(selected));
            }
            let target = config
                .targets
                .get(&selected)
                .ok_or_else(|| OperatorError::TargetNotFound(selected.clone()))?;
            Ok(serde_json::json!({
                "target": {
                    "name": selected,
                    "is_default": config.default_target == TargetId(selected.clone()),
                    "platform": target.platform,
                    "driver": target.driver,
                    "description": target.description,
                    "driver_config": target.driver_config,
                }
            }))
        }
        TargetCommand::Use { name, .. } => {
            if !document.has_persisted_named_target(name) {
                return Err(OperatorError::TargetNotFound(name.clone()));
            }
            document.set_default_target(&TargetId(name.clone()));
            validate_and_save_target_document(&document)?;
            Ok(serde_json::json!({
                "default_target": name,
                "message": format!("default target set to {name}"),
            }))
        }
        TargetCommand::Set { name, entries, .. } => {
            for entry in entries {
                let (path, value) = parse_target_set_expression(entry)?;
                document.set_target_value(name, &path, value)?;
            }
            let validated = validate_and_save_target_document(&document)?;
            let target = validated
                .targets
                .get(name)
                .ok_or_else(|| OperatorError::TargetNotFound(name.clone()))?;
            Ok(serde_json::json!({
                "target": {
                    "name": name,
                    "is_default": validated.default_target == TargetId(name.clone()),
                    "platform": target.platform,
                    "driver": target.driver,
                    "description": target.description,
                    "driver_config": target.driver_config,
                },
                "message": format!("updated target {name}"),
            }))
        }
        TargetCommand::Unset { name, paths, .. } => {
            if !document.has_persisted_named_target(name) {
                return Err(OperatorError::TargetNotFound(name.clone()));
            }
            for path in paths {
                let parsed = TargetConfigFieldPath::parse_unset(path)?;
                document.unset_target_value(name, &parsed)?;
            }
            let validated = validate_and_save_target_document(&document)?;
            let target = validated
                .targets
                .get(name)
                .ok_or_else(|| OperatorError::TargetNotFound(name.clone()))?;
            Ok(serde_json::json!({
                "target": {
                    "name": name,
                    "is_default": validated.default_target == TargetId(name.clone()),
                    "platform": target.platform,
                    "driver": target.driver,
                    "description": target.description,
                    "driver_config": target.driver_config,
                },
                "message": format!("updated target {name}"),
            }))
        }
        TargetCommand::Remove { name, .. } => {
            if !document.has_persisted_named_target(name) {
                return Err(OperatorError::TargetNotFound(name.clone()));
            }
            if config.default_target == TargetId(name.clone()) {
                return Err(OperatorError::Platform(format!(
                    "cannot remove target `{name}` while it is the default target"
                )));
            }
            document.remove_named_target(name);
            validate_and_save_target_document(&document)?;
            Ok(serde_json::json!({
                "removed_target": name,
                "message": format!("removed target {name}"),
            }))
        }
    }
}

pub(crate) fn inspect_model_command(
    command: &ModelCommand,
    operator_home: impl AsRef<Path>,
) -> Result<Value, OperatorError> {
    let path = runtime_config_path(operator_home);
    let mut document = RuntimeConfigDocument::load(&path)?;
    let bootstrap = document.to_bootstrap_config()?;

    match command {
        ModelCommand::List { .. } => {
            let selectors = configured_model_selectors(&bootstrap.agent_model);
            Ok(serde_json::json!({
                "default_selector": bootstrap.agent_model.default,
                "models": selectors
                    .iter()
                    .map(|selector| model_payload(selector, &bootstrap.agent_model))
                    .collect::<Vec<_>>(),
            }))
        }
        ModelCommand::Show { name, .. } => {
            let selector = name
                .clone()
                .or_else(|| bootstrap.agent_model.default.clone())
                .ok_or_else(|| {
                    OperatorError::Platform(
                        "no default model selector configured; use `operator model show <name>` to inspect a selector explicitly".into(),
                    )
                })?;
            Ok(serde_json::json!({
                "default_selector": bootstrap.agent_model.default,
                "model": model_payload(&selector, &bootstrap.agent_model),
            }))
        }
        ModelCommand::Use { name, .. } => {
            document.set_default_model_selector(name)?;
            let validated = validate_and_save_model_document(&document)?;
            Ok(serde_json::json!({
                "default_selector": validated.agent_model.default,
                "model": model_payload(name, &validated.agent_model),
                "message": format!("default model selector set to {name}"),
            }))
        }
        ModelCommand::Set { name, entries, .. } => {
            for entry in entries {
                let (path, value) = parse_model_set_expression(entry)?;
                document.set_model_provider_value(name, &path, value)?;
            }
            let validated = validate_and_save_model_document(&document)?;
            Ok(serde_json::json!({
                "default_selector": validated.agent_model.default,
                "model": model_payload(name, &validated.agent_model),
                "message": format!("updated model selector {name}"),
            }))
        }
        ModelCommand::Unset { name, paths, .. } => {
            let removed_default_provider_entry = bootstrap.agent_model.default.as_deref()
                == Some(name)
                && bootstrap.agent_model.providers.contains_key(name);
            for path in paths {
                let parsed = ModelConfigFieldPath::parse_unset(path)?;
                document.unset_model_provider_value(name, &parsed)?;
            }
            let validated = document.to_bootstrap_config()?;
            if removed_default_provider_entry && !validated.agent_model.providers.contains_key(name)
            {
                return Err(OperatorError::Platform(format!(
                    "cannot remove provider entry `{name}` while it is the configured default selector"
                )));
            }
            document.save()?;
            Ok(serde_json::json!({
                "default_selector": validated.agent_model.default,
                "model": model_payload(name, &validated.agent_model),
                "message": format!("updated model selector {name}"),
            }))
        }
    }
}

fn validate_and_save_target_document(
    document: &RuntimeConfigDocument,
) -> Result<operator_runtime::RuntimeConfig, OperatorError> {
    let config = document.to_runtime_config()?;
    if !config.targets.contains_key(&config.default_target.0) {
        return Err(OperatorError::Platform(format!(
            "default target `{}` is not defined under [targets]",
            config.default_target
        )));
    }
    document.save()?;
    Ok(config)
}

fn validate_and_save_model_document(
    document: &RuntimeConfigDocument,
) -> Result<operator_bootstrap::BootstrapConfig, OperatorError> {
    let config = document.to_bootstrap_config()?;
    document.save()?;
    Ok(config)
}

fn format_operator_error(error: &OperatorError) -> String {
    match error {
        OperatorError::TargetNotFound(target) => format!(
            "target not found: {target}. Use 'operator target list' to inspect configured names, 'operator target show <name>' to inspect one target, or 'operator target use <name>' to change the default target."
        ),
        _ => error.to_string(),
    }
}

fn configured_model_selectors(agent_model: &AgentModelConfig) -> Vec<String> {
    let mut selectors = BTreeSet::new();
    if let Some(default) = &agent_model.default {
        selectors.insert(default.clone());
    }
    selectors.extend(agent_model.providers.keys().cloned());
    selectors.into_iter().collect()
}

fn model_payload(selector: &str, agent_model: &AgentModelConfig) -> Value {
    let provider = agent_model.providers.get(selector);
    let api_kind = provider
        .and_then(|provider| provider.api_kind.clone())
        .or_else(|| {
            default_model_api_kind_for_selector(selector)
                .ok()
                .map(str::to_owned)
        });
    serde_json::json!({
        "name": selector,
        "is_default": agent_model.default.as_deref() == Some(selector),
        "provider_kind": model_provider_kind(selector),
        "api_kind": api_kind,
        "model_name": provider.and_then(|provider| provider.model_name.clone()),
        "base_url": provider.and_then(|provider| provider.base_url.clone()),
        "api_key": output::mask_secret(provider.and_then(|provider| provider.api_key.as_deref())),
    })
}

fn model_provider_kind(selector: &str) -> &'static str {
    match selector {
        "openai" => "openai",
        "doubao" => "doubao",
        _ => "unknown",
    }
}

#[cfg(test)]
struct NoopAgentExecutor;

#[cfg(test)]
impl AgentExecutor for NoopAgentExecutor {
    fn run<'a>(&'a self, _command: &'a AgentCommand) -> AgentFuture<'a> {
        Box::pin(async move { Err("unexpected agent execution in tool-only test".to_string()) })
    }
}

#[cfg(test)]
struct NoopTargetInspector;

#[cfg(test)]
impl TargetInspector for NoopTargetInspector {
    fn inspect<'a>(&'a self, _command: &'a TargetCommand) -> InspectFuture<'a> {
        Box::pin(async move {
            Err(OperatorError::Platform(
                "unexpected target inspection in tool-only test".into(),
            ))
        })
    }
}

#[cfg(test)]
struct NoopModelInspector;

#[cfg(test)]
impl ModelInspector for NoopModelInspector {
    fn inspect<'a>(&'a self, _command: &'a ModelCommand) -> InspectFuture<'a> {
        Box::pin(async move {
            Err(OperatorError::Platform(
                "unexpected model inspection in tool-only test".into(),
            ))
        })
    }
}

#[cfg(test)]
pub(crate) async fn run_with_handlers(
    cli: Cli,
    invoker: &impl ToolInvoker,
    executor: &impl AgentExecutor,
    target_inspector: &impl TargetInspector,
    model_inspector: &impl ModelInspector,
) -> Result<String, CliError> {
    let execution = cli.into_execution().map_err(CliError::Argument)?;
    match execution {
        CliExecution::Tool(invocation) => run_invocation_with_invoker(invocation, invoker)
            .await
            .map_err(CliError::Operator),
        CliExecution::Target(command) => run_target_with_inspector(command, target_inspector)
            .await
            .map_err(CliError::Operator),
        CliExecution::Model(command) => run_model_with_inspector(command, model_inspector)
            .await
            .map_err(CliError::Operator),
        CliExecution::Agent(command) => run_agent_with_executor(command, executor)
            .await
            .map_err(CliError::Agent),
        CliExecution::McpServe => Err(CliError::Argument(
            "mcp serve is not supported by the test helper".to_string(),
        )),
    }
}

#[cfg(test)]
pub(crate) async fn run_with_invoker(
    cli: Cli,
    invoker: &impl ToolInvoker,
) -> Result<String, CliError> {
    run_with_handlers(
        cli,
        invoker,
        &NoopAgentExecutor,
        &NoopTargetInspector,
        &NoopModelInspector,
    )
    .await
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) enum CliError {
    Argument(String),
    Operator(OperatorError),
    Agent(String),
}

#[cfg(test)]
impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Argument(message) => f.write_str(message),
            Self::Operator(error) => f.write_str(&format_operator_error(error)),
            Self::Agent(message) => f.write_str(message),
        }
    }
}
