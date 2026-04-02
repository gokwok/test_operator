#![cfg_attr(test, allow(dead_code))]

use console::style;
use operator_agent::{AgentProgressEvent, AgentRunResult};
use serde_json::{json, Value};

const MAX_PROGRESS_TEXT_CHARS: usize = 96;

pub(crate) fn render_success(tool: &str, output: &Value, json_output: bool) -> String {
    if json_output {
        return serde_json::to_string_pretty(output).expect("tool output should be valid JSON");
    }

    match tool {
        "observe" | "snapshot-get" => render_snapshot(output),
        "artifact-get" => render_artifact(output),
        "target-list" => render_targets(output),
        "target-show" => render_target(output),
        "target-use" | "target-set" | "target-unset" | "target-remove" => {
            render_target_mutation(output)
        }
        "model-list" => render_models(output),
        "model-show" => render_model(output),
        "model-use" | "model-set" | "model-unset" => render_model_mutation(output),
        "get-focus" => render_focus(output),
        "list-apps" => render_apps(output),
        "list-windows" => render_windows(output),
        "permissions-status" => render_permissions(output),
        "capabilities" => render_capabilities(output),
        "click" | "move" | "type" | "press" | "scroll" | "drag" | "swipe" | "hotkey"
        | "launch-app" | "focus-window" | "close-window" | "minimize-window"
        | "maximize-window" | "move-window" | "resize-window" | "set-window-bounds"
        | "switch-app" | "quit-app" | "relaunch-app" | "hide-app" | "unhide-app" => {
            render_action(output)
        }
        _ => serde_json::to_string_pretty(output).expect("tool output should be valid JSON"),
    }
}

pub(crate) fn mask_secret(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let visible = value.chars().count();
    if visible <= 4 {
        return Some(value.to_owned());
    }

    let suffix = value
        .chars()
        .skip(visible.saturating_sub(4))
        .collect::<String>();
    Some(format!("{}{}", "*".repeat(visible - 4), suffix))
}

pub(crate) fn render_error(json_output: bool, message: &str) -> String {
    if json_output {
        return serde_json::to_string_pretty(&json!({
            "error": {
                "message": message
            }
        }))
        .expect("error payload should serialize");
    }

    format!("error: {message}")
}

pub(crate) fn render_agent_success(result: &AgentRunResult, json_output: bool) -> String {
    if json_output {
        return serde_json::to_string_pretty(result)
            .expect("agent run result should serialize to JSON");
    }

    // In human mode the session header and RunCompleted event already carry all
    // this information, so we suppress the redundant trailing block.
    String::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressSection {
    Setup,
    Turn(u32),
}

#[derive(Debug, Default)]
pub(crate) struct AgentProgressRenderer {
    current_section: Option<ProgressSection>,
    /// The most recently seen tool call — used to synthesise a confirmation
    /// line when the result carries no meaningful summary (e.g. "result: outcome").
    last_tool_call: Option<(String, serde_json::Value)>,
}

impl AgentProgressRenderer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn render(&mut self, event: &AgentProgressEvent) -> Option<String> {
        let mut lines = Vec::new();

        match event {
            AgentProgressEvent::RunStarted {
                session_id,
                target,
                model,
                task,
            } => {
                self.current_section = None;
                let session_id_str = session_id.to_string();
                let short_id = abbreviate_session_id(&session_id_str);
                lines.push(format!(
                    "{} {}  {}",
                    style("◆").cyan().bold(),
                    style(format!("target={target}")).bold(),
                    style(format!("model={model}")).dim(),
                ));
                lines.push(format!(
                    "  {}  {}",
                    compact_progress_text(task),
                    style(format!("({short_id})")).dim(),
                ));
            }
            AgentProgressEvent::TurnStarted { turn_index } => {
                self.enter_turn(&mut lines, *turn_index);
            }
            AgentProgressEvent::PlannedTool {
                turn_index,
                tool_name: _,
                summary,
            } => {
                self.enter_progress_section(&mut lines, *turn_index);
                let n = current_turn(&self.current_section).unwrap_or(*turn_index);
                lines.push(thinking_line(n, summary));
            }
            AgentProgressEvent::FinishPlanned {
                turn_index,
                summary,
            } => {
                self.enter_progress_section(&mut lines, *turn_index);
                let n = current_turn(&self.current_section).unwrap_or(*turn_index);
                lines.push(thinking_line(n, summary));
            }
            AgentProgressEvent::ToolCall {
                turn_index,
                step_index: _,
                name,
                args,
            } => {
                self.enter_progress_section(&mut lines, *turn_index);
                self.last_tool_call = Some((name.clone(), args.clone()));
                // This line is consumed by the spinner in ConsoleAgentProgressReporter;
                // rendering it here keeps the renderer testable and state consistent.
                let label = tool_call_label(name, args);
                lines.push(format!(
                    "  {} {}",
                    style(&label).yellow().bold(),
                    style("…").dim()
                ));
            }
            AgentProgressEvent::ToolResult {
                turn_index,
                step_index: _,
                name,
                summary,
                is_error,
            } => {
                self.enter_progress_section(&mut lines, *turn_index);
                let last_call = self.last_tool_call.take();

                // Observe / snapshot / list-apps calls are internal plumbing —
                // suppress their result lines entirely.
                if is_observe_tool(name) || name == "list-apps" {
                    return None;
                }

                if *is_error {
                    // Strip redundant "error [...]:" / "platform error:" prefixes.
                    let msg = strip_error_prefix(summary);
                    lines.push(result_line(false, true, compact_progress_text(msg)));
                } else {
                    match meaningful_summary(summary) {
                        Some(text) => {
                            lines.push(result_line(false, false, compact_progress_text(text)));
                        }
                        None => {
                            // Generic "result: outcome" — show tool name + args instead.
                            if let Some((tool_name, tool_args)) = last_call {
                                let label = tool_call_label(&tool_name, &tool_args);
                                lines.push(result_line(true, false, label));
                            }
                        }
                    }
                }
            }
            AgentProgressEvent::FinishGateRejected { turn_index, reason } => {
                self.enter_progress_section(&mut lines, *turn_index);
                lines.push(format!(
                    "      {} {}",
                    style("⏸").yellow(),
                    compact_progress_text(reason)
                ));
            }
            AgentProgressEvent::RunCompleted { summary } => {
                lines.push(String::new());
                lines.push(format!(
                    "{} {}",
                    style("✓").green().bold(),
                    style(compact_progress_text(summary)).green().bold()
                ));
            }
            AgentProgressEvent::RunFailed { reason } => {
                lines.push(String::new());
                lines.push(format!(
                    "{} {}",
                    style("✗").red().bold(),
                    style(compact_progress_text(reason)).red().bold()
                ));
            }
        }

        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn enter_progress_section(&mut self, lines: &mut Vec<String>, turn_index: u32) {
        if turn_index == 0 {
            if self.current_section != Some(ProgressSection::Setup) {
                lines.push(format!("{}", style("  setup").dim()));
                self.current_section = Some(ProgressSection::Setup);
            }
            return;
        }

        self.enter_turn(lines, turn_index);
    }

    fn enter_turn(&mut self, lines: &mut Vec<String>, turn_index: u32) {
        let section = ProgressSection::Turn(turn_index);
        if self.current_section != Some(section) {
            lines.push(String::new()); // blank line between turns for breathing room
            self.current_section = Some(section);
        }
    }
}

fn render_snapshot(output: &Value) -> String {
    // Try to deserialize and render a rich element tree; fall back to a
    // one-line summary when the payload cannot be parsed into a Snapshot.
    if let Ok(snapshot) =
        serde_json::from_value::<operator_core::Snapshot>(output["snapshot"].clone())
    {
        snapshot.render_element_tree(&operator_core::DigestOptions {
            max_entries: 200,
            ..Default::default()
        })
    } else {
        let snapshot = &output["snapshot"];
        let id = snapshot["id"].as_str().unwrap_or("<unknown>");
        let target = snapshot["target"].as_str().unwrap_or("<unknown>");
        let warning = snapshot["metadata"]["element_tree"]["note"]
            .as_str()
            .map(|note| format!(" [element tree warning: {note}]"))
            .unwrap_or_default();
        format!("snapshot {id} ({target}){warning}")
    }
}

/// Layout constants for the thinking / result column alignment.
///
/// Thinking line:  `"  {N:>2}  ∘ {text}"`
///   prefix width  = 2 (indent) + 2 (number) + 2 (gap) = 6 chars before the symbol.
///
/// Result line:    `"      {sym} {text}"`
///   prefix width  = 6 spaces, symbol aligns with ∘.
const RESULT_INDENT: &str = "      "; // 6 spaces

/// Formats the thinking line for a turn: `"   1  ∘ text"`.
fn thinking_line(turn_index: u32, summary: &str) -> String {
    format!(
        "  {:>2}  {} {}",
        style(turn_index.to_string()).dim(),
        style("∘").cyan(),
        style(compact_progress_text(summary)).dim()
    )
}

/// Formats a result line aligned under the thinking ∘ symbol.
/// `dim_label` applies dim styling (used for fallback tool+args labels).
fn result_line(dim_label: bool, is_error: bool, text: String) -> String {
    if is_error {
        format!(
            "{}{} {}",
            RESULT_INDENT,
            style("✗").red(),
            style(text).red()
        )
    } else if dim_label {
        format!(
            "{}{} {}",
            RESULT_INDENT,
            style("✓").green(),
            style(text).dim()
        )
    } else {
        format!("{}{} {}", RESULT_INDENT, style("✓").green(), text)
    }
}

/// Extracts the turn index from the current section state.
fn current_turn(section: &Option<ProgressSection>) -> Option<u32> {
    match section {
        Some(ProgressSection::Turn(n)) => Some(*n),
        _ => None,
    }
}

/// Builds the spinner / renderer label for a tool call: `"click  x=450 y=320"`.
/// Falls back to just the tool name when no meaningful args can be extracted.
pub(crate) fn tool_call_label(name: &str, args: &serde_json::Value) -> String {
    match format_tool_args(name, args) {
        Some(formatted) => format!("{name}  {formatted}"),
        None => name.to_string(),
    }
}

/// Extracts (x, y) from a normalized Locator JSON value.
/// Supports all Locator variants produced by the normalizer:
/// `{"Coords": {"x", "y"}}`,
/// `{"SnapshotPixelCoords": {"snapshot", "point": {"x","y"}}}`,
/// `{"SnapshotCoords": …}`, `{"SnapshotNormalizedCoords": …}`.
fn extract_locator_coords(locator: &serde_json::Value) -> Option<(f64, f64)> {
    if let Some(p) = locator.get("Coords") {
        return Some((p["x"].as_f64()?, p["y"].as_f64()?));
    }
    for key in [
        "SnapshotPixelCoords",
        "SnapshotCoords",
        "SnapshotNormalizedCoords",
    ] {
        if let Some(sp) = locator.get(key) {
            return Some((sp["point"]["x"].as_f64()?, sp["point"]["y"].as_f64()?));
        }
    }
    None
}

/// Formats a Locator as a human-readable label (coords or text).
fn format_locator(locator: &serde_json::Value) -> Option<String> {
    if let Some((x, y)) = extract_locator_coords(locator) {
        return Some(format!("x={} y={}", render_number(x), render_number(y)));
    }
    if let Some(text) = locator.get("Text").and_then(|v| v.as_str()) {
        let preview: String = text.chars().take(30).collect();
        let suffix = if text.chars().count() > 30 { "…" } else { "" };
        return Some(format!("\"{preview}{suffix}\""));
    }
    if let Some(obj) = locator.get("Role").and_then(|v| v.as_object()) {
        if let Some(role) = obj.get("role").and_then(|v| v.as_str()) {
            return Some(format!("[{role}]"));
        }
    }
    None
}

/// Extracts the most user-visible arguments for a given tool call.
fn format_tool_args(name: &str, args: &serde_json::Value) -> Option<String> {
    let s = match name {
        // Pointer actions: show coordinates or text from the locator
        "click" | "move" => {
            let locator = &args["locator"];
            format_locator(locator)?
        }
        // Text input: show the text (truncated)
        "type" => {
            let text = args["text"].as_str()?;
            let preview: String = text.chars().take(40).collect();
            let suffix = if text.chars().count() > 40 { "…" } else { "" };
            format!("\"{}{}\"", preview, suffix)
        }
        // Key presses
        "press" => {
            let key = args["key"].as_str()?;
            let count = args["count"].as_u64().unwrap_or(1);
            if count > 1 {
                format!("{key} ×{count}")
            } else {
                key.to_string()
            }
        }
        // Hotkey combos
        "hotkey" => {
            let keys = args["keys"]
                .as_array()?
                .iter()
                .filter_map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join("+");
            keys
        }
        // App lifecycle
        "launch-app" | "switch-app" | "quit-app" | "relaunch-app" | "hide-app" | "unhide-app" => {
            // Try bundle_id_or_name first, then bundle_id, then name
            args["bundle_id_or_name"]
                .as_str()
                .or_else(|| args["bundle_id"].as_str())
                .or_else(|| args["name"].as_str())
                .map(ToOwned::to_owned)?
        }
        // Scroll: direction + amount
        "scroll" => {
            let dx = args["delta_x"].as_f64().unwrap_or(0.0);
            let dy = args["delta_y"].as_f64().unwrap_or(0.0);
            let dir = if dy < 0.0 {
                "↑"
            } else if dy > 0.0 {
                "↓"
            } else if dx < 0.0 {
                "←"
            } else {
                "→"
            };
            format!("{dir} dx={} dy={}", render_number(dx), render_number(dy))
        }
        // Drag / swipe: from → to (both are Locator values)
        "drag" | "swipe" => {
            let from = format_locator(&args["from"])?;
            let to = format_locator(&args["to"])?;
            format!("{from} → {to}")
        }
        _ => return None,
    };
    Some(s)
}

/// Strips redundant error prefixes like `"error [platform]: platform error: "` that
/// wrap the real message but add no actionable information for the user.
fn strip_error_prefix(msg: &str) -> &str {
    let mut s = msg.trim();
    // Strip "error [<tag>]: " wrapper
    if let Some(rest) = s.strip_prefix("error [") {
        if let Some(after_tag) = rest.find("]: ") {
            s = rest[after_tag + 3..].trim();
        }
    }
    // Strip "platform error: " prefix
    if let Some(rest) = s.strip_prefix("platform error: ") {
        s = rest.trim();
    }
    s
}

/// Returns true for tools whose results are internal plumbing (observe / snapshots).
/// Their result lines add pure technical noise and are suppressed from human output.
fn is_observe_tool(name: &str) -> bool {
    matches!(name, "observe" | "snapshot-get")
}

/// Returns `None` when the summary carries no user-visible information
/// (e.g. the generic `"result: outcome"` produced by action tools), otherwise
/// strips the redundant `"result: "` prefix and returns the remaining text.
fn meaningful_summary(summary: &str) -> Option<&str> {
    let text = summary.strip_prefix("result: ").unwrap_or(summary).trim();
    if text.is_empty() || text == "outcome" {
        return None;
    }
    Some(text)
}

/// Abbreviates a session ID for compact display.
/// `"agent-1775026438846-35920-1"` → `"35920-1"`
fn abbreviate_session_id(id: &str) -> &str {
    // Drop everything up to and including the second `-`-separated numeric group
    // (the millisecond timestamp), leaving the pid+counter suffix that is both
    // short and sufficient to distinguish concurrent sessions.
    let mut dashes = 0u8;
    for (i, ch) in id.char_indices() {
        if ch == '-' {
            dashes += 1;
            if dashes == 2 {
                return &id[i + 1..];
            }
        }
    }
    id // fallback: return whole ID if format is unexpected
}

fn compact_progress_text(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_progress_text(&normalized)
}

fn truncate_progress_text(text: &str) -> String {
    let mut chars = text.chars();
    let truncated = chars
        .by_ref()
        .take(MAX_PROGRESS_TEXT_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn render_artifact(output: &Value) -> String {
    let artifact = &output["artifact"];
    let id = artifact["id"].as_str().unwrap_or("<unknown>");
    let path = artifact["path"].as_str().unwrap_or("<unknown>");
    format!("artifact {id} ({path})")
}

fn render_focus(output: &Value) -> String {
    let focus = &output["focus"];
    if focus.is_null() {
        return "no focused element".into();
    }

    let role = focus["role"].as_str().unwrap_or("<unknown>");
    let label = focus["label"].as_str();
    let app = focus["app_name"]
        .as_str()
        .or_else(|| focus["bundle_id"].as_str())
        .unwrap_or("<unknown>");

    match label {
        Some(label) if !label.is_empty() => format!("{app}\t{role}\t{label}"),
        _ => format!("{app}\t{role}"),
    }
}

fn render_targets(output: &Value) -> String {
    let targets = output["targets"].as_array().cloned().unwrap_or_default();
    if targets.is_empty() {
        return "no configured targets".into();
    }

    let rendered = targets
        .iter()
        .map(|target| {
            let name = target["name"].as_str().unwrap_or("<unknown>");
            let mut heading = format!("  • {name}");
            if target["is_default"].as_bool().unwrap_or(false) {
                heading.push_str(" [default]");
            }

            let mut block = format!(
                "{heading}\n    Platform: {}\n    Driver: {}",
                target["platform"].as_str().unwrap_or("<unknown>"),
                target["driver"].as_str().unwrap_or("<unknown>")
            );
            if let Some(description) = target["description"]
                .as_str()
                .filter(|value| !value.is_empty())
            {
                block.push_str(&format!("\n    Description: {description}"));
            }
            block
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("Targets ({}):\n{rendered}", targets.len())
}

fn render_target(output: &Value) -> String {
    let target = &output["target"];
    if target.is_null() {
        return "target not found".into();
    }

    let mut lines = vec![
        format!("Target: {}", target["name"].as_str().unwrap_or("<unknown>")),
        format!(
            "Default: {}",
            if target["is_default"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "Platform: {}",
            target["platform"].as_str().unwrap_or("<unknown>")
        ),
        format!(
            "Driver: {}",
            target["driver"].as_str().unwrap_or("<unknown>")
        ),
    ];
    if let Some(description) = target["description"]
        .as_str()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Description: {description}"));
    }
    let driver_config = serde_json::to_string_pretty(&target["driver_config"])
        .expect("driver_config should serialize");
    lines.push(format!(
        "Driver Config:\n{}",
        indent_block(&driver_config, "  ")
    ));
    lines.join("\n")
}

fn render_target_mutation(output: &Value) -> String {
    output["message"]
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            serde_json::to_string_pretty(output).expect("mutation output should serialize")
        })
}

fn render_model_mutation(output: &Value) -> String {
    output["message"]
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            serde_json::to_string_pretty(output).expect("mutation output should serialize")
        })
}

fn render_models(output: &Value) -> String {
    let models = output["models"].as_array().cloned().unwrap_or_default();
    if models.is_empty() {
        return "no configured models".into();
    }

    let rendered = models
        .iter()
        .map(|model| {
            let name = model["name"].as_str().unwrap_or("<unknown>");
            let mut heading = format!("  • {name}");
            if model["is_default"].as_bool().unwrap_or(false) {
                heading.push_str(" [default]");
            }

            format!(
                "{heading}\n    Provider: {}\n    API Kind: {}\n    Model: {}\n    Base URL: {}\n    API Key: {}",
                render_string_field(model["provider_kind"].as_str()),
                render_string_field(model["api_kind"].as_str()),
                render_string_field(model["model_name"].as_str()),
                render_string_field(model["base_url"].as_str()),
                render_string_field(model["api_key"].as_str()),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("Models ({}):\n{rendered}", models.len())
}

fn render_model(output: &Value) -> String {
    let model = &output["model"];
    if model.is_null() {
        return "model not found".into();
    }

    [
        format!("Model: {}", model["name"].as_str().unwrap_or("<unknown>")),
        format!(
            "Default: {}",
            if model["is_default"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "Provider: {}",
            render_string_field(model["provider_kind"].as_str())
        ),
        format!(
            "API Kind: {}",
            render_string_field(model["api_kind"].as_str())
        ),
        format!(
            "Model Name: {}",
            render_string_field(model["model_name"].as_str())
        ),
        format!(
            "Base URL: {}",
            render_string_field(model["base_url"].as_str())
        ),
        format!(
            "API Key: {}",
            render_string_field(model["api_key"].as_str())
        ),
    ]
    .join("\n")
}

fn render_apps(output: &Value) -> String {
    let apps = output["apps"].as_array().cloned().unwrap_or_default();
    if apps.is_empty() {
        return "no apps".into();
    }

    let heading = if apps
        .iter()
        .all(|app| app["is_running"].as_bool().unwrap_or(false))
    {
        "Running Applications"
    } else {
        "Applications"
    };

    let rendered = apps
        .iter()
        .map(|app| {
            let name = app["name"].as_str().unwrap_or("<unknown>");
            let bundle = app["bundle_id"].as_str().unwrap_or("unknown");
            let status_or_pid = match app["pid"].as_u64() {
                Some(pid) => format!("PID: {pid}"),
                None if app["is_running"].as_bool() == Some(false) => "Status: installed".into(),
                None => "Status: running".into(),
            };

            format!("  • {name}\n    Bundle: {bundle}\n    {status_or_pid}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{heading} ({}):\n{rendered}", apps.len())
}

fn render_windows(output: &Value) -> String {
    let windows = output["windows"].as_array().cloned().unwrap_or_default();
    if windows.is_empty() {
        return "no windows".into();
    }

    windows
        .iter()
        .map(|window| {
            let id = window["id"].as_u64().unwrap_or_default();
            let title = window["title"].as_str().unwrap_or("<untitled>");
            let app = window["app_name"].as_str().unwrap_or("<unknown>");
            format!("{id}\t{app}\t{title}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_permissions(output: &Value) -> String {
    let checks = output["permissions"]["checks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if checks.is_empty() {
        return "no permission checks".into();
    }

    let mut rendered = checks
        .iter()
        .map(|check| {
            let label = check["label"].as_str().unwrap_or("<unknown>");
            let status = check["status"].as_str().unwrap_or("Unknown");
            format!("{label}: {status}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    for note in checks.iter().filter_map(|check| {
        if check["status"].as_str() == Some("Granted") {
            return None;
        }

        check["message"].as_str()
    }) {
        rendered.push_str(&format!("\nnote: {note}"));
    }

    rendered
}

fn render_capabilities(output: &Value) -> String {
    let capabilities = output["capabilities"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if capabilities.is_empty() {
        return "no capabilities".into();
    }

    capabilities
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_action(output: &Value) -> String {
    let outcome = &output["outcome"];
    if let Some(detail) = outcome["detail"].as_str() {
        return detail.to_string();
    }

    if let Some(detail) = render_action_from_structured(outcome) {
        return detail;
    }

    if outcome["success"].as_bool().unwrap_or(false) {
        "ok".into()
    } else {
        "failed".into()
    }
}

fn render_action_from_structured(outcome: &Value) -> Option<String> {
    let effect = outcome["side_effects"].as_array()?.first()?;
    let kind = effect["kind"].as_str()?;

    match kind {
        "Click" => Some(match effect["data"]["mode"].as_str()? {
            "Left" => "clicked".into(),
            "Right" => "right-clicked".into(),
            "Middle" => "middle-clicked".into(),
            "Double" => "double-clicked".into(),
            _ => return None,
        }),
        "MoveCursor" => Some("moved".into()),
        "Type" => Some("typed text".into()),
        "Press" => {
            let key = effect["data"]["key"].as_str()?;
            let count = effect["data"]["count"].as_u64()?;
            Some(if count == 1 {
                format!("pressed {key}")
            } else {
                format!("pressed {key} {count} times")
            })
        }
        "Scroll" => Some("scrolled".into()),
        "Hotkey" => Some("sent hotkey".into()),
        "Drag" => Some("dragged".into()),
        "Swipe" => Some("swiped".into()),
        "LaunchApp" => Some("launched".into()),
        "FocusWindow" => outcome["target_window"]["id"]
            .as_u64()
            .map(|id| format!("focused window {id}")),
        "CloseWindow" => outcome["target_window"]["id"]
            .as_u64()
            .map(|id| format!("closed window {id}")),
        "MinimizeWindow" => outcome["target_window"]["id"]
            .as_u64()
            .map(|id| format!("minimized window {id}")),
        "MaximizeWindow" => outcome["target_window"]["id"]
            .as_u64()
            .map(|id| format!("maximized window {id}")),
        "MoveWindow" => render_window_geometry(outcome, "moved"),
        "ResizeWindow" => render_window_geometry(outcome, "resized"),
        "SetWindowBounds" => render_window_bounds(outcome),
        "SwitchApp" => Some("switched app".into()),
        "QuitApp" => Some("quit app".into()),
        "RelaunchApp" => Some("relaunched app".into()),
        "HideApp" => Some("hid app".into()),
        "UnhideApp" => Some("unhid app".into()),
        _ => None,
    }
}

fn render_window_geometry(outcome: &Value, verb: &str) -> Option<String> {
    let id = outcome["target_window"]["id"].as_u64()?;
    let bounds = &outcome["target_window"]["bounds"];
    Some(format!(
        "{verb} window {id} to x={} y={} width={} height={}",
        render_number(bounds["x"].as_f64()?),
        render_number(bounds["y"].as_f64()?),
        render_number(bounds["width"].as_f64()?),
        render_number(bounds["height"].as_f64()?),
    ))
}

fn render_window_bounds(outcome: &Value) -> Option<String> {
    let id = outcome["target_window"]["id"].as_u64()?;
    let bounds = &outcome["target_window"]["bounds"];
    Some(format!(
        "set window {id} bounds to x={} y={} width={} height={}",
        render_number(bounds["x"].as_f64()?),
        render_number(bounds["y"].as_f64()?),
        render_number(bounds["width"].as_f64()?),
        render_number(bounds["height"].as_f64()?),
    ))
}

fn render_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn render_string_field(value: Option<&str>) -> &str {
    value.filter(|value| !value.is_empty()).unwrap_or("<unset>")
}

fn indent_block(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        mask_secret, render_apps, render_model, render_models, render_permissions, render_snapshot,
        render_target, render_targets, AgentProgressRenderer,
    };
    use operator_agent::AgentProgressEvent;

    #[test]
    fn render_apps_uses_multiline_blocks_for_running_entries() {
        let output = json!({
            "apps": [
                {
                    "bundle_id": "company.thebrowser.Browser",
                    "name": "Arc",
                    "pid": 2366,
                    "is_running": true
                },
                {
                    "bundle_id": "com.openai.codex",
                    "name": "Codex",
                    "pid": 2834,
                    "is_running": true
                }
            ]
        });

        assert_eq!(
            render_apps(&output),
            "Running Applications (2):\n  • Arc\n    Bundle: company.thebrowser.Browser\n    PID: 2366\n  • Codex\n    Bundle: com.openai.codex\n    PID: 2834"
        );
    }

    #[test]
    fn render_apps_marks_non_running_entries_with_status() {
        let output = json!({
            "apps": [
                {
                    "bundle_id": "com.apple.Calculator",
                    "name": "Calculator",
                    "pid": null,
                    "is_running": false
                },
                {
                    "bundle_id": null,
                    "name": "Codex",
                    "pid": 2834,
                    "is_running": true
                }
            ]
        });

        assert_eq!(
            render_apps(&output),
            "Applications (2):\n  • Calculator\n    Bundle: com.apple.Calculator\n    Status: installed\n  • Codex\n    Bundle: unknown\n    PID: 2834"
        );
    }

    #[test]
    fn render_permissions_includes_system_events_status() {
        let output = json!({
            "permissions": {
                "checks": [
                    {
                        "id": "accessibility",
                        "label": "Accessibility",
                        "status": "Granted",
                        "message": "Accessibility permission is required for macOS automation."
                    },
                    {
                        "id": "system_events",
                        "label": "System Events",
                        "status": "Granted",
                        "message": "System Events access is required for macOS window queries and focus reads."
                    },
                    {
                        "id": "screen_recording",
                        "label": "Screen Recording",
                        "status": "Denied",
                        "message": "Screen Recording permission is required for macOS capture."
                    }
                ]
            }
        });

        assert_eq!(
            render_permissions(&output),
            "Accessibility: Granted\nSystem Events: Granted\nScreen Recording: Denied\nnote: Screen Recording permission is required for macOS capture."
        );
    }

    #[test]
    fn render_permissions_adds_note_when_system_events_diverges() {
        let output = json!({
            "permissions": {
                "checks": [
                    {
                        "id": "accessibility",
                        "label": "Accessibility",
                        "status": "Granted",
                        "message": "Accessibility permission is required for macOS automation."
                    },
                    {
                        "id": "system_events",
                        "label": "System Events",
                        "status": "Denied",
                        "message": "System Events access is required for macOS window queries and focus reads."
                    },
                    {
                        "id": "screen_recording",
                        "label": "Screen Recording",
                        "status": "Granted",
                        "message": "Screen Recording permission is required for macOS capture."
                    }
                ]
            }
        });

        assert_eq!(
            render_permissions(&output),
            "Accessibility: Granted\nSystem Events: Denied\nScreen Recording: Granted\nnote: System Events access is required for macOS window queries and focus reads."
        );
    }

    #[test]
    fn render_targets_includes_default_marker_and_optional_description() {
        let output = json!({
            "targets": [
                {
                    "name": "harmony-pc",
                    "is_default": true,
                    "platform": "harmony",
                    "driver": "harmony.hdc",
                    "description": "Harmony lab PC"
                },
                {
                    "name": "macos",
                    "is_default": false,
                    "platform": "macos",
                    "driver": "macos.system",
                    "description": null
                }
            ]
        });

        assert_eq!(
            render_targets(&output),
            "Targets (2):\n  • harmony-pc [default]\n    Platform: harmony\n    Driver: harmony.hdc\n    Description: Harmony lab PC\n  • macos\n    Platform: macos\n    Driver: macos.system"
        );
    }

    #[test]
    fn render_target_pretty_prints_driver_config() {
        let output = json!({
            "target": {
                "name": "harmony-pc",
                "is_default": true,
                "platform": "harmony",
                "driver": "harmony.hdc",
                "description": "Harmony lab PC",
                "driver_config": {
                    "addr": "192.168.8.43:35319"
                }
            }
        });

        assert_eq!(
            render_target(&output),
            "Target: harmony-pc\nDefault: yes\nPlatform: harmony\nDriver: harmony.hdc\nDescription: Harmony lab PC\nDriver Config:\n  {\n    \"addr\": \"192.168.8.43:35319\"\n  }"
        );
    }

    #[test]
    fn mask_secret_keeps_only_the_last_four_visible_characters() {
        assert_eq!(
            mask_secret(Some("sk-openai-1234")).as_deref(),
            Some("**********1234")
        );
        assert_eq!(mask_secret(Some("1234")).as_deref(), Some("1234"));
        assert_eq!(mask_secret(Some("   ")), None);
    }

    #[test]
    fn render_models_shows_default_marker_and_masked_secret() {
        let output = json!({
            "models": [
                {
                    "name": "openai",
                    "is_default": true,
                    "provider_kind": "openai",
                    "api_kind": "responses",
                    "model_name": "gpt-5.4",
                    "base_url": "https://api.openai.com/v1",
                    "api_key": "**********1234"
                },
                {
                    "name": "doubao",
                    "is_default": false,
                    "provider_kind": "doubao",
                    "api_kind": "chat_completions",
                    "model_name": null,
                    "base_url": null,
                    "api_key": null
                }
            ]
        });

        assert_eq!(
            render_models(&output),
            "Models (2):\n  • openai [default]\n    Provider: openai\n    API Kind: responses\n    Model: gpt-5.4\n    Base URL: https://api.openai.com/v1\n    API Key: **********1234\n  • doubao\n    Provider: doubao\n    API Kind: chat_completions\n    Model: <unset>\n    Base URL: <unset>\n    API Key: <unset>"
        );
    }

    #[test]
    fn render_model_prints_standardized_selector_shape() {
        let output = json!({
            "model": {
                "name": "doubao",
                "is_default": false,
                "provider_kind": "doubao",
                "api_kind": "chat_completions",
                "model_name": "doubao-seed-2-0-lite-260215",
                "base_url": "https://ark.cn-beijing.volces.com/api/v3",
                "api_key": "********5678"
            }
        });

        assert_eq!(
            render_model(&output),
            "Model: doubao\nDefault: no\nProvider: doubao\nAPI Kind: chat_completions\nModel Name: doubao-seed-2-0-lite-260215\nBase URL: https://ark.cn-beijing.volces.com/api/v3\nAPI Key: ********5678"
        );
    }

    #[test]
    fn render_snapshot_appends_element_tree_warning_when_present() {
        let output = json!({
            "snapshot": {
                "id": "snapshot-1",
                "target": "harmony-pc",
                "metadata": {
                    "element_tree": {
                        "reliability": "unreliable",
                        "note": "Harmony element tree is too sparse for reliable no-vision interaction on this screen; prefer pure-vision (screenshot-only) mode."
                    }
                }
            }
        });

        assert_eq!(
            render_snapshot(&output),
            "snapshot snapshot-1 (harmony-pc) [element tree warning: Harmony element tree is too sparse for reliable no-vision interaction on this screen; prefer pure-vision (screenshot-only) mode.]"
        );
    }

    #[test]
    fn progress_renderer_prints_setup_turn_plan_tool_and_completion_lines() {
        let mut renderer = AgentProgressRenderer::new();

        assert_eq!(
            renderer.render(&AgentProgressEvent::RunStarted {
                session_id: operator_core::SessionId("agent-7".into()),
                target: operator_core::TargetId("macos".into()),
                model: "openai".into(),
                task: "Open Calculator and compute 114 x 9999.".into(),
            }),
            Some(
                "◆ target=macos  model=openai\n  Open Calculator and compute 114 x 9999.  (agent-7)".into()
            )
        );
        assert_eq!(
            renderer.render(&AgentProgressEvent::ToolCall {
                turn_index: 0,
                step_index: 0,
                name: "observe".into(),
                args: serde_json::Value::Null,
            }),
            Some("  setup\n  observe …".into())
        );
        assert_eq!(
            renderer.render(&AgentProgressEvent::TurnStarted { turn_index: 1 }),
            Some("".into())
        );
        assert_eq!(
            renderer.render(&AgentProgressEvent::PlannedTool {
                turn_index: 1,
                tool_name: "launch-app".into(),
                summary: "Launch Calculator before typing.".into(),
            }),
            Some("   1  ∘ Launch Calculator before typing.".into())
        );
        assert_eq!(
            renderer.render(&AgentProgressEvent::ToolResult {
                turn_index: 1,
                step_index: 1,
                name: "launch-app".into(),
                summary: "action succeeded".into(),
                is_error: false,
            }),
            Some("      ✓ action succeeded".into())
        );
        // Observe results are suppressed entirely.
        assert_eq!(
            renderer.render(&AgentProgressEvent::ToolResult {
                turn_index: 1,
                step_index: 2,
                name: "observe".into(),
                summary: "snapshot snapshot-123 (macos)".into(),
                is_error: false,
            }),
            None
        );
        // Generic "result: outcome" with no preceding ToolCall → None.
        assert_eq!(
            renderer.render(&AgentProgressEvent::ToolResult {
                turn_index: 1,
                step_index: 3,
                name: "click".into(),
                summary: "result: outcome".into(),
                is_error: false,
            }),
            None
        );
        // Generic "result: outcome" WITH a preceding ToolCall → show tool+args.
        renderer.render(&AgentProgressEvent::ToolCall {
            turn_index: 1,
            step_index: 4,
            name: "click".into(),
            args: serde_json::json!({"locator": {"Coords": {"x": 450, "y": 320}}}),
        });
        assert_eq!(
            renderer.render(&AgentProgressEvent::ToolResult {
                turn_index: 1,
                step_index: 4,
                name: "click".into(),
                summary: "result: outcome".into(),
                is_error: false,
            }),
            Some("      ✓ click  x=450 y=320".into())
        );
        assert_eq!(
            renderer.render(&AgentProgressEvent::RunCompleted {
                summary: "The calculator now shows 1139886.".into(),
            }),
            Some("\n✓ The calculator now shows 1139886.".into())
        );
    }

    #[test]
    fn progress_renderer_compacts_multiline_text_and_truncates_long_messages() {
        let mut renderer = AgentProgressRenderer::new();
        let rendered = renderer
            .render(&AgentProgressEvent::RunStarted {
                session_id: operator_core::SessionId("agent-9".into()),
                target: operator_core::TargetId("macos".into()),
                model: "doubao".into(),
                task: format!(
                    "First line with spacing.\nSecond line with more text. {}",
                    "x".repeat(120)
                ),
            })
            .expect("run start should render");

        assert!(rendered.contains("First line with spacing. Second line with more text."));
        assert!(rendered.contains("..."));
    }
}
