use std::{io, sync::Arc};

use operator_bootstrap::{load_runtime_config, operator_home_dir, system_platform_registry};
use operator_runtime::{FileArtifactStore, FileSnapshotStore, RuntimeBuilder};

pub mod server;
pub mod transport_stdio;

pub use server::McpServer;
pub use transport_stdio::{run_stdio_session, StdioTransportError};

pub async fn run_stdio_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_runtime_config()?;
    let root = operator_home_dir();
    let snapshots = Arc::new(FileSnapshotStore::new(&root, config.clone()));
    let artifacts = Arc::new(FileArtifactStore::new(&root));
    let runtime = RuntimeBuilder::new(config)
        .artifact_store(artifacts.clone())
        .snapshot_store(snapshots)
        .platform_registry(system_platform_registry(artifacts.artifacts_dir()))
        .build()
        .await?;
    let runtime_config = runtime.core().config().clone();
    let server = McpServer::new(runtime.tools().clone())
        .with_allow_side_effects(runtime_config.allow_side_effects)
        .with_default_target(runtime_config.default_target.clone())
        .with_default_timeout_ms(runtime_config.default_timeout_ms);
    let stdin = io::stdin();
    let stdout = io::stdout();

    run_stdio_session(&server, stdin.lock(), &mut stdout.lock())?;
    Ok(())
}
