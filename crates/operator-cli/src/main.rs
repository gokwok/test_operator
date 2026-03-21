#![cfg_attr(test, allow(dead_code))]

pub(crate) mod args;
mod output;

use std::{env, future::Future, path::PathBuf, pin::Pin, sync::Arc};

use operator_core::OperatorError;
use operator_platform_macos::MacosDriver;
use operator_runtime::{FileSnapshotStore, RuntimeBuilder, RuntimeConfig, ToolRegistry};
use serde_json::Value;

use self::args::Cli;

#[cfg(not(test))]
use clap::Parser;

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
        let snapshots = Arc::new(FileSnapshotStore::new(operator_home_dir(), config.clone()));
        let runtime = RuntimeBuilder::new(config)
            .snapshot_store(snapshots)
            .register_driver(Arc::new(MacosDriver::system()))
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

    let invoker = match RuntimeToolInvoker::build().await {
        Ok(invoker) => invoker,
        Err(error) => {
            eprintln!("{}", output::render_error(json_output, &error.to_string()));
            return 1;
        }
    };

    match run_with_invoker(cli, &invoker).await {
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

pub(crate) async fn run_with_invoker(
    cli: Cli,
    invoker: &impl ToolInvoker,
) -> Result<String, CliError> {
    let invocation = cli.into_invocation().map_err(CliError::Argument)?;
    let output = invoker
        .invoke(invocation.tool, invocation.input)
        .await
        .map_err(CliError::Operator)?;

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

#[derive(Debug)]
pub(crate) enum CliError {
    Argument(String),
    Operator(OperatorError),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Argument(message) => f.write_str(message),
            Self::Operator(error) => write!(f, "{error}"),
        }
    }
}
