use std::{collections::HashSet, env, path::PathBuf, process::ExitCode, sync::Arc};

use clap::Parser;
use operator_agent::{
    load_persisted_session,
    model::{
        DoubaoChatCompletionsProvider, DoubaoProviderConfig, ModelRegistry, OpenAiProviderConfig,
        OpenAiResponsesProvider, ProviderKind,
    },
    AgentConfig, AgentRunRequest, AgentRunResult, AgentRunner, PersistedSessionTranscript,
    ReplayableTranscriptEvent,
};
use operator_core::TargetId;
use operator_platform_macos::{
    MacosDriver, SystemAppService, SystemCaptureProvider, SystemPermissionReader,
    SystemTreeInspector,
};
use operator_runtime::{
    FileArtifactStore, FileSessionStore, FileSnapshotStore, RuntimeBuilder, RuntimeConfig,
    SessionStore,
};
use serde_json::Value;

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
struct Cli {
    #[arg(long, help = "Task prompt to send into the phase-1 agent loop")]
    task: String,

    #[arg(
        long,
        default_value = "local:macos",
        help = "Target id to run against, for example local:macos"
    )]
    target: String,

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

struct HarnessReport {
    request: AgentRunRequest,
    state_root: PathBuf,
    result: Option<AgentRunResult>,
    failure: Option<String>,
    transcript: Option<PersistedSessionTranscript>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli).await {
        Ok(report) => {
            print!("{}", render_report(&report));

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
    let state_root = cli.state_root.unwrap_or_else(default_state_root);
    let target = TargetId(cli.target.clone());
    let request = AgentRunRequest {
        task: cli.task,
        target: target.clone(),
        model: Some(cli.model.clone()),
    };

    let runtime_config = RuntimeConfig {
        default_target: target,
        ..RuntimeConfig::default()
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

    Ok(HarnessReport {
        request,
        state_root,
        result,
        failure,
        transcript,
    })
}

async fn build_runtime(
    state_root: &PathBuf,
    config: RuntimeConfig,
    session_store: Arc<FileSessionStore>,
) -> Result<operator_runtime::Runtime, String> {
    let artifacts = Arc::new(FileArtifactStore::new(state_root));
    let snapshots = Arc::new(FileSnapshotStore::new(state_root, config.clone()));
    let capture_provider = SystemCaptureProvider::new(artifacts.artifacts_dir());

    RuntimeBuilder::new(config)
        .artifact_store(artifacts)
        .snapshot_store(snapshots)
        .session_store(session_store)
        .register_driver(Arc::new(MacosDriver::with_observe(
            SystemAppService,
            SystemPermissionReader,
            capture_provider,
            SystemTreeInspector,
        )))
        .build()
        .await
        .map_err(|error| error.to_string())
}

fn configured_models() -> Result<ModelRegistry, String> {
    let mut registry = ModelRegistry::new();
    let mut configured = Vec::new();

    if let Some(api_key) = non_empty_env("OPENAI_API_KEY") {
        let mut config = OpenAiProviderConfig::new(api_key);
        if let Some(base_url) = non_empty_env("OPENAI_BASE_URL") {
            config.base_url = base_url;
        }
        let provider = OpenAiResponsesProvider::new(config).map_err(|error| error.to_string())?;
        registry.register_provider(ProviderKind::OpenAi, Arc::new(provider));
        configured.push("OPENAI_API_KEY");
    }

    let doubao_api_key = non_empty_env("ARK_API_KEY").or_else(|| non_empty_env("DOUBAO_API_KEY"));
    if let Some(api_key) = doubao_api_key {
        let mut config = DoubaoProviderConfig::new(api_key);
        if let Some(base_url) =
            non_empty_env("ARK_BASE_URL").or_else(|| non_empty_env("DOUBAO_BASE_URL"))
        {
            config.base_url = base_url;
        }
        let provider =
            DoubaoChatCompletionsProvider::new(config).map_err(|error| error.to_string())?;
        registry.register_provider(ProviderKind::OpenAiCompatible, Arc::new(provider));
        configured.push("ARK_API_KEY/DOUBAO_API_KEY");
    }

    if configured.is_empty() {
        return Err(
            "no model provider credentials found; set OPENAI_API_KEY or ARK_API_KEY before running the harness"
                .into(),
        );
    }

    Ok(registry)
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .and_then(|value| if value.is_empty() { None } else { Some(value) })
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

fn render_report(report: &HarnessReport) -> String {
    let mut sections = vec![
        render_final_result(report),
        render_transcript(report.transcript.as_ref()),
        render_tool_trace(report.transcript.as_ref()),
    ];
    sections.retain(|section| !section.trim().is_empty());
    sections.join("\n\n")
}

fn render_final_result(report: &HarnessReport) -> String {
    let mut lines = vec![
        "== Final Result ==".to_string(),
        format!("task: {}", report.request.task),
        format!("target: {}", report.request.target),
        format!(
            "requested_model: {}",
            report.request.model.as_deref().unwrap_or("default")
        ),
        format!("state_root: {}", report.state_root.display()),
    ];

    if let Some(result) = &report.result {
        lines.push(format!("session_id: {}", result.session_id));
        lines.push(format!("resolved_model: {}", result.model));
        lines.push(format!("summary: {}", result.summary));
    } else {
        lines.push("session_id: unavailable".into());
    }

    if let Some(error) = &report.failure {
        lines.push(format!("error: {error}"));
    }

    lines.join("\n")
}

fn render_transcript(transcript: Option<&PersistedSessionTranscript>) -> String {
    let Some(transcript) = transcript else {
        return "== Transcript ==\n(unavailable)".into();
    };

    let mut lines = vec![
        "== Transcript ==".to_string(),
        format!("persisted_session: {}", transcript.session.id),
    ];

    for (index, event) in transcript.events.iter().enumerate() {
        lines.push(format!("[{}] {}", index + 1, describe_event(event)));
        if let Some(body) = event_body(event) {
            lines.push(body);
        }
    }

    lines.join("\n")
}

fn render_tool_trace(transcript: Option<&PersistedSessionTranscript>) -> String {
    let Some(transcript) = transcript else {
        return "== Tool Trace ==\n(unavailable)".into();
    };

    let entries = transcript
        .events
        .iter()
        .filter_map(|event| match event {
            ReplayableTranscriptEvent::ToolResult { result } => Some(result),
            _ => None,
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return "== Tool Trace ==\n(no tool results recorded)".into();
    }

    let mut lines = vec!["== Tool Trace ==".to_string()];
    for (index, entry) in entries.iter().enumerate() {
        lines.push(format!(
            "[{}] {} status={} read_only={}",
            index + 1,
            entry.tool_name,
            if entry.is_error { "error" } else { "ok" },
            entry.read_only
        ));
        lines.push(format!("arguments:\n{}", render_json(&entry.arguments)));
        if let Some(output) = &entry.output {
            lines.push(format!("output:\n{}", render_json(output)));
        }
        if let Some(error) = &entry.error {
            lines.push(format!(
                "error:\n{}",
                render_json(&serde_json::json!(error))
            ));
        }
    }

    lines.join("\n")
}

fn describe_event(event: &ReplayableTranscriptEvent) -> String {
    match event {
        ReplayableTranscriptEvent::UserInput { .. } => "user_input".into(),
        ReplayableTranscriptEvent::ToolCall { name, .. } => format!("tool_call {name}"),
        ReplayableTranscriptEvent::ToolResult { result } => {
            format!("tool_result {}", result.tool_name)
        }
        ReplayableTranscriptEvent::ModelResponse { .. } => "model_response".into(),
        ReplayableTranscriptEvent::Completed { .. } => "completed".into(),
        ReplayableTranscriptEvent::Error { .. } => "error".into(),
    }
}

fn event_body(event: &ReplayableTranscriptEvent) -> Option<String> {
    match event {
        ReplayableTranscriptEvent::UserInput { text } => Some(text.clone()),
        ReplayableTranscriptEvent::ToolCall { input, .. } => Some(render_json(input)),
        ReplayableTranscriptEvent::ToolResult { result } => {
            Some(render_json(&serde_json::json!(result)))
        }
        ReplayableTranscriptEvent::ModelResponse { content } => Some(content.clone()),
        ReplayableTranscriptEvent::Completed { summary } => {
            Some(summary.clone().unwrap_or_else(|| "(no summary)".into()))
        }
        ReplayableTranscriptEvent::Error { message } => Some(message.clone()),
    }
}

fn render_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn default_state_root() -> PathBuf {
    if let Some(path) = env::var_os("OPERATOR_HOME") {
        return PathBuf::from(path).join("agent-harness");
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".operator").join("agent-harness");
    }

    PathBuf::from(".operator").join("agent-harness")
}
