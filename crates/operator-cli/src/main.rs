#![cfg_attr(test, allow(dead_code))]

pub(crate) mod args;
mod output;

use std::{future::Future, path::Path, pin::Pin, sync::Arc};

use operator_agent::{
    model::ModelRegistry, AgentConfig, AgentRunRequest, AgentRunResult, AgentRunner,
};
use operator_bootstrap::{load_runtime_config, operator_home_dir, system_platform_registry};
use operator_core::OperatorError;
#[cfg(not(test))]
use operator_mcp::run_stdio_server;
use operator_runtime::{
    FileArtifactStore, FileSessionStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig,
    ToolRegistry,
};
use serde_json::Value;

use self::args::{AgentCommand, Cli, CliExecution, ToolInvocation};

type InvokeFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, OperatorError>> + Send + 'a>>;
type AgentFuture<'a> = Pin<Box<dyn Future<Output = Result<AgentRunResult, String>> + Send + 'a>>;

pub(crate) trait ToolInvoker {
    fn invoke<'a>(&'a self, tool: &'a str, input: Value) -> InvokeFuture<'a>;
}

pub(crate) trait AgentExecutor {
    fn run<'a>(&'a self, command: &'a AgentCommand) -> AgentFuture<'a>;
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
                    eprintln!("{}", output::render_error(json_output, &error.to_string()));
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

#[cfg(test)]
struct NoopAgentExecutor;

#[cfg(test)]
impl AgentExecutor for NoopAgentExecutor {
    fn run<'a>(&'a self, _command: &'a AgentCommand) -> AgentFuture<'a> {
        Box::pin(async move { Err("unexpected agent execution in tool-only test".to_string()) })
    }
}

#[cfg(test)]
pub(crate) async fn run_with_handlers(
    cli: Cli,
    invoker: &impl ToolInvoker,
    executor: &impl AgentExecutor,
) -> Result<String, CliError> {
    let execution = cli.into_execution().map_err(CliError::Argument)?;
    match execution {
        CliExecution::Tool(invocation) => run_invocation_with_invoker(invocation, invoker)
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
    run_with_handlers(cli, invoker, &NoopAgentExecutor).await
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
            Self::Operator(error) => write!(f, "{error}"),
            Self::Agent(message) => f.write_str(message),
        }
    }
}
