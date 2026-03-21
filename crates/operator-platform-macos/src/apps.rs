use std::process::Command;

use operator_core::{AppInfo, OperatorError, Rect, WindowId, WindowInfo};
use serde::Deserialize;

pub trait AppService: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppInfo>, OperatorError>;
    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError>;
    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemAppService;

impl AppService for SystemAppService {
    fn list_apps(&self) -> Result<Vec<AppInfo>, OperatorError> {
        let script = r#"
const systemEvents = Application("System Events");
const apps = systemEvents.applicationProcesses().map(function(process) {
  return {
    bundle_id: null,
    name: process.name(),
    pid: process.unixId(),
    is_running: true
  };
});
JSON.stringify(apps);
"#;

        let apps: Vec<AppRecord> = parse_jxa_json(run_jxa(script)?)?;
        Ok(apps.into_iter().map(AppInfo::from).collect())
    }

    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError> {
        let app_literal = serde_json::to_string(&app).map_err(|error| {
            OperatorError::Platform(format!("failed to encode app filter: {error}"))
        })?;
        let script = format!(
            r#"
const filter = {app_literal};
const systemEvents = Application("System Events");
const processes = filter
  ? systemEvents.applicationProcesses.whose({{name: filter}})()
  : systemEvents.applicationProcesses();
let windows = [];
for (const process of processes) {{
  const appName = process.name();
  const isFrontmost = process.frontmost();
  for (const window of process.windows()) {{
    windows.push({{
      id: Number(window.id()),
      title: window.name() || null,
      app_name: appName,
      bounds: null,
      is_focused: Boolean(isFrontmost),
      is_minimized: false
    }});
  }}
}}
JSON.stringify(windows);
"#
        );

        let windows: Vec<WindowRecord> = parse_jxa_json(run_jxa(&script)?)?;
        Ok(windows.into_iter().map(WindowInfo::from).collect())
    }

    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        launch_with_open(bundle_id_or_name)
    }
}

#[derive(Debug, Deserialize)]
struct AppRecord {
    bundle_id: Option<String>,
    name: String,
    pid: Option<u32>,
    is_running: bool,
}

impl From<AppRecord> for AppInfo {
    fn from(value: AppRecord) -> Self {
        Self {
            bundle_id: value.bundle_id,
            name: value.name,
            pid: value.pid,
            is_running: value.is_running,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WindowRecord {
    id: u64,
    title: Option<String>,
    app_name: Option<String>,
    bounds: Option<Rect>,
    is_focused: bool,
    is_minimized: bool,
}

impl From<WindowRecord> for WindowInfo {
    fn from(value: WindowRecord) -> Self {
        Self {
            id: WindowId::from(value.id),
            title: value.title,
            app_name: value.app_name,
            bounds: value.bounds,
            is_focused: value.is_focused,
            is_minimized: value.is_minimized,
        }
    }
}

fn parse_jxa_json<T>(json: String) -> Result<T, OperatorError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(&json).map_err(|error| {
        OperatorError::Platform(format!("failed to decode macOS command output: {error}"))
    })
}

#[cfg(target_os = "macos")]
fn run_jxa(script: &str) -> Result<String, OperatorError> {
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .map_err(|error| OperatorError::Platform(format!("failed to invoke osascript: {error}")))?;

    command_output("osascript", output)
}

#[cfg(not(target_os = "macos"))]
fn run_jxa(_script: &str) -> Result<String, OperatorError> {
    Err(OperatorError::Platform(
        "macOS app queries are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn launch_with_open(bundle_id_or_name: &str) -> Result<(), OperatorError> {
    let mut command = Command::new("open");
    if bundle_id_or_name.contains('.') {
        command.args(["-b", bundle_id_or_name]);
    } else {
        command.args(["-a", bundle_id_or_name]);
    }

    let output = command
        .output()
        .map_err(|error| OperatorError::Platform(format!("failed to invoke open: {error}")))?;

    command_output("open", output).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn launch_with_open(_bundle_id_or_name: &str) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS app launch is unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn command_output(command: &str, output: std::process::Output) -> Result<String, OperatorError> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("Not authorized") || stderr.contains("not allowed") {
        return Err(OperatorError::PermissionDenied(stderr));
    }

    Err(OperatorError::Platform(format!(
        "{command} failed: {stderr}"
    )))
}
