#![cfg_attr(test, allow(dead_code))]

use operator_agent::AgentRunResult;
use serde_json::{json, Value};

pub(crate) fn render_success(tool: &str, output: &Value, json_output: bool) -> String {
    if json_output {
        return serde_json::to_string_pretty(output).expect("tool output should be valid JSON");
    }

    match tool {
        "observe" | "snapshot-get" => render_snapshot(output),
        "artifact-get" => render_artifact(output),
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

    format!(
        "session_id: {}\ntarget: {}\nmodel: {}\nsummary: {}",
        result.session_id, result.target, result.model, result.summary
    )
}

fn render_snapshot(output: &Value) -> String {
    let snapshot = &output["snapshot"];
    let id = snapshot["id"].as_str().unwrap_or("<unknown>");
    let target = snapshot["target"].as_str().unwrap_or("<unknown>");
    format!("snapshot {id} ({target})")
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

fn render_apps(output: &Value) -> String {
    let apps = output["apps"].as_array().cloned().unwrap_or_default();
    if apps.is_empty() {
        return "no apps".into();
    }

    apps.iter()
        .map(|app| {
            let name = app["name"].as_str().unwrap_or("<unknown>");
            match app["pid"].as_u64() {
                Some(pid) => format!("{name}\tpid={pid}"),
                None if app["is_running"].as_bool() == Some(false) => {
                    format!("{name}\tinstalled")
                }
                None => name.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::render_permissions;

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
}
