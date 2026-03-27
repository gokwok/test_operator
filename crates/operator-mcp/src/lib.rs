use std::{env, io, path::PathBuf, sync::Arc};

use operator_bootstrap::system_platform_registry;
use operator_runtime::{FileArtifactStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig};

pub mod server;
pub mod transport_stdio;

pub use server::McpServer;
pub use transport_stdio::{run_stdio_session, StdioTransportError};

pub async fn run_stdio_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = RuntimeConfig::default();
    let root = operator_home_dir();
    let snapshots = Arc::new(FileSnapshotStore::new(&root, config.clone()));
    let artifacts = Arc::new(FileArtifactStore::new(&root));
    let runtime = RuntimeBuilder::new(config)
        .artifact_store(artifacts.clone())
        .snapshot_store(snapshots)
        .platform_registry(system_platform_registry(artifacts.artifacts_dir()))
        .build()
        .await?;
    let server = McpServer::new(runtime.tools().clone());
    let stdin = io::stdin();
    let stdout = io::stdout();

    run_stdio_session(&server, stdin.lock(), &mut stdout.lock())?;
    Ok(())
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
