use std::{env, io, path::PathBuf, sync::Arc};

use operator_mcp::{run_stdio_session, McpServer};
use operator_platform_macos::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};
use operator_runtime::{FileArtifactStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig};

#[tokio::main]
async fn main() {
    std::process::exit(match run().await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    });
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
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
