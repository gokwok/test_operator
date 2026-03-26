use std::path::PathBuf;

use operator_core::{ArtifactId, Snapshot, SnapshotId};
use serde_json::Value;

use crate::{
    AgentRunRequest, AgentRunResult, PersistedSessionTranscript, ReplayableTranscriptEvent,
    VisualObservationSummary,
};

#[derive(Debug, Clone)]
pub struct HarnessReport {
    pub request: AgentRunRequest,
    pub state_root: PathBuf,
    pub result: Option<AgentRunResult>,
    pub failure: Option<String>,
    pub transcript: Option<PersistedSessionTranscript>,
}

impl HarnessReport {
    pub fn new(
        request: AgentRunRequest,
        state_root: PathBuf,
        result: Option<AgentRunResult>,
        failure: Option<String>,
        transcript: Option<PersistedSessionTranscript>,
    ) -> Self {
        Self {
            request,
            state_root,
            result,
            failure,
            transcript,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessReplaySummary {
    pub persisted_session_id: Option<String>,
    pub replayable_event_count: usize,
    pub tool_result_count: usize,
    pub observation_count: usize,
    pub current_snapshot_id: Option<SnapshotId>,
    pub current_visual_artifact: Option<ArtifactId>,
    pub previous_snapshot_id: Option<SnapshotId>,
    pub previous_visual_artifact: Option<ArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessTimingSummary {
    pub measurement_count: usize,
    pub total_duration_ms: u64,
    pub by_tool: Vec<ToolTimingSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTimingSummary {
    pub tool_name: String,
    pub measurement_kind: String,
    pub count: usize,
    pub total_duration_ms: u64,
    pub average_duration_ms: u64,
    pub max_duration_ms: u64,
}

#[derive(Debug, Clone)]
struct ToolMeasurement {
    tool_name: String,
    measurement_kind: &'static str,
    duration_ms: u64,
}

pub fn render_harness_report(report: &HarnessReport) -> String {
    let mut sections = vec![
        render_final_result(report),
        render_replay_summary(summarize_transcript_replay(report.transcript.as_ref())),
        render_timing_summary(summarize_timing(report.transcript.as_ref())),
        render_transcript(report.transcript.as_ref()),
        render_tool_trace(report.transcript.as_ref()),
    ];
    sections.retain(|section| !section.trim().is_empty());
    sections.join("\n\n")
}

pub fn summarize_transcript_replay(
    transcript: Option<&PersistedSessionTranscript>,
) -> Option<HarnessReplaySummary> {
    let transcript = transcript?;
    let observations = observed_visuals(transcript);
    let current = observations.last().cloned();
    let previous = observations.iter().rev().nth(1).cloned();

    Some(HarnessReplaySummary {
        persisted_session_id: Some(transcript.session.id.to_string()),
        replayable_event_count: transcript.events.len(),
        tool_result_count: transcript
            .events
            .iter()
            .filter(|event| matches!(event, ReplayableTranscriptEvent::ToolResult { .. }))
            .count(),
        observation_count: observations.len(),
        current_snapshot_id: current.as_ref().map(|summary| summary.snapshot_id.clone()),
        current_visual_artifact: current
            .as_ref()
            .and_then(|summary| summary.screenshot_artifact.clone()),
        previous_snapshot_id: previous.as_ref().map(|summary| summary.snapshot_id.clone()),
        previous_visual_artifact: previous
            .as_ref()
            .and_then(|summary| summary.screenshot_artifact.clone()),
    })
}

pub fn summarize_timing(
    transcript: Option<&PersistedSessionTranscript>,
) -> Option<HarnessTimingSummary> {
    let transcript = transcript?;
    let mut measurements = transcript
        .events
        .iter()
        .filter_map(|event| match event {
            ReplayableTranscriptEvent::ToolResult { result } => {
                measurement_from_tool_result(result)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    measurements.sort_by(|left, right| {
        left.tool_name
            .cmp(&right.tool_name)
            .then(left.measurement_kind.cmp(right.measurement_kind))
            .then(left.duration_ms.cmp(&right.duration_ms))
    });

    let mut by_tool = Vec::new();
    let mut index = 0;
    while index < measurements.len() {
        let measurement = &measurements[index];
        let mut count = 0usize;
        let mut total_duration_ms = 0u64;
        let mut max_duration_ms = 0u64;
        while index < measurements.len()
            && measurements[index].tool_name == measurement.tool_name
            && measurements[index].measurement_kind == measurement.measurement_kind
        {
            let current = &measurements[index];
            count += 1;
            total_duration_ms += current.duration_ms;
            max_duration_ms = max_duration_ms.max(current.duration_ms);
            index += 1;
        }

        by_tool.push(ToolTimingSummary {
            tool_name: measurement.tool_name.clone(),
            measurement_kind: measurement.measurement_kind.to_string(),
            count,
            total_duration_ms,
            average_duration_ms: total_duration_ms / count as u64,
            max_duration_ms,
        });
    }

    Some(HarnessTimingSummary {
        measurement_count: measurements.len(),
        total_duration_ms: measurements.iter().map(|item| item.duration_ms).sum(),
        by_tool,
    })
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

fn render_replay_summary(summary: Option<HarnessReplaySummary>) -> String {
    let Some(summary) = summary else {
        return "== Replay Summary ==\n(unavailable)".into();
    };

    let lines = vec![
        "== Replay Summary ==".to_string(),
        format!(
            "persisted_session: {}",
            summary
                .persisted_session_id
                .as_deref()
                .unwrap_or("(unavailable)")
        ),
        format!("replayable_events: {}", summary.replayable_event_count),
        format!("tool_results: {}", summary.tool_result_count),
        format!("observations: {}", summary.observation_count),
        format!(
            "current_snapshot: {}",
            summary
                .current_snapshot_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(unavailable)".into())
        ),
        format!(
            "current_visual_artifact: {}",
            summary
                .current_visual_artifact
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(unavailable)".into())
        ),
        format!(
            "previous_snapshot: {}",
            summary
                .previous_snapshot_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(unavailable)".into())
        ),
        format!(
            "previous_visual_artifact: {}",
            summary
                .previous_visual_artifact
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(unavailable)".into())
        ),
    ];
    lines.join("\n")
}

fn render_timing_summary(summary: Option<HarnessTimingSummary>) -> String {
    let Some(summary) = summary else {
        return "== Timing Summary ==\n(unavailable)".into();
    };

    let mut lines = vec![
        "== Timing Summary ==".to_string(),
        format!("measurements: {}", summary.measurement_count),
        format!("total_duration_ms: {}", summary.total_duration_ms),
    ];

    if summary.by_tool.is_empty() {
        lines.push("details: (no measured durations recorded)".into());
        return lines.join("\n");
    }

    for (index, entry) in summary.by_tool.iter().enumerate() {
        lines.push(format!(
            "[{}] {} {} count={} total_ms={} avg_ms={} max_ms={}",
            index + 1,
            entry.tool_name,
            entry.measurement_kind,
            entry.count,
            entry.total_duration_ms,
            entry.average_duration_ms,
            entry.max_duration_ms
        ));
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

fn observed_visuals(transcript: &PersistedSessionTranscript) -> Vec<VisualObservationSummary> {
    transcript
        .events
        .iter()
        .filter_map(|event| match event {
            ReplayableTranscriptEvent::ToolResult { result } => result
                .output
                .as_ref()
                .and_then(snapshot_from_output)
                .map(|snapshot| VisualObservationSummary::from_snapshot(&snapshot)),
            _ => None,
        })
        .collect()
}

fn measurement_from_tool_result(result: &crate::tools::AgentToolResult) -> Option<ToolMeasurement> {
    if result.is_error {
        return None;
    }

    let output = result.output.as_ref()?;
    if let Some(snapshot) = snapshot_from_output(output) {
        return Some(ToolMeasurement {
            tool_name: result.tool_name.clone(),
            measurement_kind: "capture_duration_ms",
            duration_ms: snapshot.metadata.capture_duration_ms,
        });
    }

    output
        .get("duration_ms")
        .and_then(Value::as_u64)
        .map(|duration_ms| ToolMeasurement {
            tool_name: result.tool_name.clone(),
            measurement_kind: "duration_ms",
            duration_ms,
        })
}

fn snapshot_from_output(output: &Value) -> Option<Snapshot> {
    output
        .get("snapshot")
        .cloned()
        .and_then(|snapshot| serde_json::from_value(snapshot).ok())
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
