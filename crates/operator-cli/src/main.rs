#![cfg_attr(test, allow(dead_code))]

pub(crate) mod args;
mod output;

use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use operator_agent::{
    model::ModelRegistry, AgentConfig, AgentRunRequest, AgentRunResult, AgentRunner,
};
use operator_bootstrap::{
    load_runtime_config, operator_home_dir, parse_target_set_expression, runtime_config_path,
    system_platform_registry, RuntimeConfigDocument, TargetConfigFieldPath,
};
use operator_core::{OperatorError, TargetId};
#[cfg(not(test))]
use operator_mcp::run_stdio_server;
use operator_runtime::{
    FileArtifactStore, FileSessionStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig,
    ToolRegistry,
};
use serde_json::Value;

use self::args::{AgentCommand, Cli, CliExecution, TargetCommand, ToolInvocation};

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
            let runtime_config = runtime_config_for(command).map_err(|error| error.to_string())?;
            let request = AgentRunRequest {
                task: command.task.clone(),
                target: runtime_config.default_target.clone(),
                model: command.model.clone(),
            };
            let runtime = build_runtime(runtime_config)
                .await
                .map_err(|error| error.to_string())?;
            let models = ModelRegistry::from_environment().map_err(|error| error.to_string())?;
            let runner = AgentRunner::new(Arc::new(runtime), models, agent_config_for(command));
            runner.run(request).await.map_err(|error| error.to_string())
        })
    }
}

struct RuntimeTargetInspector {
    operator_home: PathBuf,
}

impl RuntimeTargetInspector {
    fn new(operator_home: PathBuf) -> Self {
        Self { operator_home }
    }
}

impl TargetInspector for RuntimeTargetInspector {
    fn inspect<'a>(&'a self, command: &'a TargetCommand) -> InspectFuture<'a> {
        Box::pin(async move { inspect_target_command(command, &self.operator_home) })
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
            let inspector = RuntimeTargetInspector::new(operator_home_dir());
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
        CliExecution::Agent(_command) => {
            let executor = RuntimeAgentExecutor;
            match run_agent_with_executor(_command, &executor).await {
                Ok(rendered) => {
                    println!("{rendered}");
                    0
                }
                Err(error) => {
                    eprintln!("{}", output::render_error(json_output, &error));
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

fn runtime_config_for(command: &AgentCommand) -> Result<RuntimeConfig, OperatorError> {
    runtime_config_for_home(command, operator_home_dir())
}

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

fn agent_config_for(command: &AgentCommand) -> AgentConfig {
    let mut config = AgentConfig::default();
    if let Some(max_steps) = command.max_steps {
        config.max_steps = max_steps.get();
    }
    if let Some(timeout_ms) = command.timeout_ms {
        config.step_timeout_ms = timeout_ms;
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

    match command {
        TargetCommand::List { .. } => Ok(serde_json::json!({
            "default_target": config.default_target.to_string(),
            "targets": config.targets.iter().map(|(name, target)| serde_json::json!({
                "name": name,
                "is_default": config.default_target == TargetId(name.clone()),
                "platform": target.platform,
                "driver": target.driver,
                "description": target.description,
            })).collect::<Vec<_>>(),
        })),
        TargetCommand::Show { name, .. } => {
            let selected = name
                .clone()
                .unwrap_or_else(|| config.default_target.to_string());
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
            if !config.targets.contains_key(name) {
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
            if !config.targets.contains_key(name) {
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
            if !config.targets.contains_key(name) {
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

fn format_operator_error(error: &OperatorError) -> String {
    match error {
        OperatorError::TargetNotFound(target) => format!(
            "target not found: {target}. Use 'operator target list' to inspect configured names, 'operator target show <name>' to inspect one target, or 'operator target use <name>' to change the default target."
        ),
        _ => error.to_string(),
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
pub(crate) async fn run_with_handlers(
    cli: Cli,
    invoker: &impl ToolInvoker,
    executor: &impl AgentExecutor,
    inspector: &impl TargetInspector,
) -> Result<String, CliError> {
    let execution = cli.into_execution().map_err(CliError::Argument)?;
    match execution {
        CliExecution::Tool(invocation) => run_invocation_with_invoker(invocation, invoker)
            .await
            .map_err(CliError::Operator),
        CliExecution::Target(command) => run_target_with_inspector(command, inspector)
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
    run_with_handlers(cli, invoker, &NoopAgentExecutor, &NoopTargetInspector).await
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
