#![cfg_attr(test, allow(dead_code))]

pub(crate) mod args;
mod output;

use std::{env, future::Future, path::PathBuf, pin::Pin, sync::Arc};

use operator_core::OperatorError;
#[cfg(not(test))]
use operator_mcp::run_stdio_server;
use operator_platform_macos::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};
use operator_runtime::{
    FileArtifactStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig, ToolRegistry,
};
use serde_json::Value;

#[cfg(not(test))]
use self::args::CliExecution;
use self::args::{Cli, ToolInvocation};

type InvokeFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, OperatorError>> + Send + 'a>>;

pub(crate) trait ToolInvoker {
    fn invoke<'a>(&'a self, tool: &'a str, input: Value) -> InvokeFuture<'a>;
}

struct RuntimeToolInvoker {
    tools: ToolRegistry,
}

impl RuntimeToolInvoker {
    async fn build() -> Result<Self, OperatorError> {
        let config = RuntimeConfig::default();
        let root = operator_home_dir();
        let snapshots = Arc::new(FileSnapshotStore::new(&root, config.clone()));
        let artifacts = Arc::new(FileArtifactStore::new(&root));
        let runtime = RuntimeBuilder::new(config)
            .artifact_store(artifacts.clone())
            .snapshot_store(snapshots)
            .register_driver(Arc::new(MacosDriver::with_observe(
                SystemAppService,
                SystemPermissionReader,
                SystemCaptureProvider::new(artifacts.artifacts_dir()),
                SystemTreeInspector,
            )))
            .build()
            .await?;

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

#[cfg(not(test))]
#[tokio::main]
async fn main() {
    std::process::exit(main_entry().await);
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
            eprintln!(
                "{}",
                output::render_error(json_output, "operator agent execution is not wired yet")
            );
            1
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

fn operator_home_dir() -> PathBuf {
    if let Some(path) = env::var_os("OPERATOR_HOME") {
        return PathBuf::from(path);
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".operator");
    }

    PathBuf::from(".operator")
}

#[cfg(test)]
pub(crate) async fn run_with_invoker(
    cli: Cli,
    invoker: &impl ToolInvoker,
) -> Result<String, CliError> {
    let invocation = cli.into_invocation().map_err(CliError::Argument)?;
    run_invocation_with_invoker(invocation, invoker)
        .await
        .map_err(CliError::Operator)
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) enum CliError {
    Argument(String),
    Operator(OperatorError),
}

#[cfg(test)]
impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Argument(message) => f.write_str(message),
            Self::Operator(error) => write!(f, "{error}"),
        }
    }
}
