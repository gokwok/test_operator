use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};

use clap::Parser;
use operator_agent::{
    load_persisted_session, model::ModelRegistry, render_harness_report, AgentConfig,
    AgentRunRequest, AgentRunner, HarnessReport,
};
use operator_bootstrap::{load_runtime_config_from, operator_home_dir, system_platform_registry};
use operator_core::TargetId;
use operator_runtime::{
    FileArtifactStore, FileSessionStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig,
    SessionStore,
};
#[derive(Debug, Parser)]
#[command(
    name = "local_run",
    about = "developer-only local agent harness for manual debugging against a real target",
    long_about = "developer-only local agent harness for manual debugging against a real target.\n\
Not a supported public CLI surface.\n\
\n\
Provider credentials:\n\
  gpt-5.4      -> OPENAI_API_KEY (optional OPENAI_BASE_URL)\n\
  doubao-seed  -> ARK_API_KEY or DOUBAO_API_KEY (optional ARK_BASE_URL or DOUBAO_BASE_URL)"
)]
pub(crate) struct Cli {
    #[arg(long, help = "Task prompt to send into the phase-1 agent loop")]
    task: String,

    #[arg(long, help = "Target id to run against, for example macos")]
    target: Option<String>,

    #[arg(
        long,
        default_value = "gpt-5.4",
        help = "Registered phase-1 model name (gpt-5.4 or doubao-seed)"
    )]
    model: String,

    #[arg(
        long,
        value_name = "PATH",
        help = "Override the state root used for snapshots, artifacts, and persisted sessions"
    )]
    state_root: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli).await {
        Ok(report) => {
            print!("{}", render_harness_report(&report));

            if let Some(message) = &report.failure {
                eprintln!("local agent run failed: {message}");
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("local harness setup failed: {error}");
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<HarnessReport, String> {
    let operator_home = operator_home_dir();
    let runtime_config = runtime_config_for_home(&cli, &operator_home)?;
    let state_root = cli
        .state_root
        .clone()
        .unwrap_or_else(|| default_state_root(&operator_home));
    let request = AgentRunRequest {
        task: cli.task.clone(),
        target: runtime_config.default_target.clone(),
        model: Some(cli.model.clone()),
    };

    let session_store = Arc::new(FileSessionStore::new(&state_root));
    let before_sessions = session_ids(&*session_store)
        .await
        .map_err(|error| format!("failed to list existing sessions: {error}"))?;
    let runtime = build_runtime(&state_root, runtime_config, session_store.clone()).await?;
    let models = configured_models()?;
    let runner = AgentRunner::new(Arc::new(runtime), models, AgentConfig::default());

    let run_result = runner.run(request.clone()).await;
    let session_id = match &run_result {
        Ok(result) => Some(result.session_id.clone()),
        Err(_) => newest_session(&*session_store, &before_sessions)
            .await
            .map_err(|error| format!("failed to discover persisted session: {error}"))?,
    };
    let transcript = match session_id {
        Some(id) => load_persisted_session(&*session_store, &id)
            .await
            .map_err(|error| format!("failed to load transcript: {error}"))?,
        None => None,
    };

    let (result, failure) = match run_result {
        Ok(result) => (Some(result), None),
        Err(error) => (None, Some(augment_failure(&cli.model, error))),
    };

    Ok(HarnessReport::new(
        request, state_root, result, failure, transcript,
    ))
}

pub(crate) fn runtime_config_for_home(
    cli: &Cli,
    operator_home: impl AsRef<Path>,
) -> Result<RuntimeConfig, String> {
    let mut config = load_runtime_config_from(operator_home).map_err(|error| error.to_string())?;
    if let Some(target) = &cli.target {
        config.default_target = TargetId(target.clone());
    }
    Ok(config)
}

async fn build_runtime(
    state_root: &PathBuf,
    config: RuntimeConfig,
    session_store: Arc<FileSessionStore>,
) -> Result<operator_runtime::Runtime, String> {
    let artifacts = Arc::new(FileArtifactStore::new(state_root));
    let snapshots = Arc::new(FileSnapshotStore::new(state_root, config.clone()));

    RuntimeBuilder::new(config)
        .artifact_store(artifacts)
        .snapshot_store(snapshots)
        .session_store(session_store)
        .platform_registry(system_platform_registry(state_root.join("artifacts")))
        .build()
        .await
        .map_err(|error| error.to_string())
}

fn configured_models() -> Result<ModelRegistry, String> {
    ModelRegistry::from_environment().map_err(|error| error.to_string())
}

async fn session_ids(
    store: &dyn SessionStore,
) -> Result<HashSet<String>, operator_core::OperatorError> {
    Ok(store
        .list(Some(100))
        .await?
        .into_iter()
        .map(|id| id.to_string())
        .collect())
}

async fn newest_session(
    store: &dyn SessionStore,
    before_sessions: &HashSet<String>,
) -> Result<Option<operator_core::SessionId>, operator_core::OperatorError> {
    let sessions = store.list(Some(100)).await?;
    Ok(sessions
        .into_iter()
        .find(|id| !before_sessions.contains(&id.to_string())))
}

fn augment_failure(model: &str, error: impl std::fmt::Display) -> String {
    match model {
        "gpt-5.4" => format!(
            "{error}. Hint: configure OPENAI_API_KEY (and optionally OPENAI_BASE_URL) for gpt-5.4."
        ),
        "doubao-seed" => format!(
            "{error}. Hint: configure ARK_API_KEY or DOUBAO_API_KEY (and optionally ARK_BASE_URL / DOUBAO_BASE_URL) for doubao-seed."
        ),
        _ => error.to_string(),
    }
}

fn default_state_root(operator_home: &Path) -> PathBuf {
    operator_home.join("agent-harness")
}
