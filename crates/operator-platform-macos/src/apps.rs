use std::process::Command;

use operator_core::{AppInfo, FocusInfo, OperatorError, Rect, WindowId, WindowInfo};
use serde::Deserialize;

pub trait AppService: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppInfo>, OperatorError>;
    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError>;
    fn get_focus(&self) -> Result<Option<FocusInfo>, OperatorError>;
    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError>;
    fn close_window(&self, id: WindowId) -> Result<(), OperatorError>;
    fn minimize_window(&self, id: WindowId) -> Result<(), OperatorError>;
    fn maximize_window(&self, id: WindowId) -> Result<(), OperatorError>;
    fn focus_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        self.launch_app(bundle_id_or_name)
    }
    fn quit_app(&self, app_name: &str) -> Result<(), OperatorError>;
    fn relaunch_app(&self, app_name: &str) -> Result<(), OperatorError> {
        self.quit_app(app_name)?;
        self.launch_app(app_name)
    }
    fn hide_app(&self, app_name: &str) -> Result<(), OperatorError>;
    fn unhide_app(&self, app_name: &str) -> Result<(), OperatorError>;
    fn focus_window(&self, id: WindowId) -> Result<(), OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemAppService;

impl AppService for SystemAppService {
    fn list_apps(&self) -> Result<Vec<AppInfo>, OperatorError> {
        let script = r#"
const systemEvents = Application("System Events");
const apps = systemEvents.applicationProcesses().map(function(process) {
  return {
    bundle_id: typeof process.bundleIdentifier === "function"
      ? process.bundleIdentifier()
      : null,
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
function safeAttr(target, name) {{
  try {{
    return target.attributes.byName(name).value();
  }} catch (error) {{
    return null;
  }}
}}
function rectForElement(element) {{
  try {{
    const position = element.position();
    const size = element.size();
    return {{
      x: Number(position[0]),
      y: Number(position[1]),
      width: Number(size[0]),
      height: Number(size[1])
    }};
  }} catch (error) {{
    return null;
  }}
}}
const processes = filter
  ? systemEvents.applicationProcesses.whose({{name: filter}})()
  : systemEvents.applicationProcesses();
let windows = [];
for (const process of processes) {{
  const appName = process.name();
  const isFrontmost = process.frontmost();
  for (const window of process.windows()) {{
    const isMain = safeAttr(window, "AXMain");
    const isFocused = safeAttr(window, "AXFocused");
    windows.push({{
      id: Number(window.id()),
      title: window.name() || null,
      app_name: appName,
      bounds: rectForElement(window),
      is_focused: Boolean(isFrontmost && (isMain || isFocused)),
      is_minimized: Boolean(safeAttr(window, "AXMinimized"))
    }});
  }}
}}
JSON.stringify(windows);
"#
        );

        let windows: Vec<WindowRecord> = parse_jxa_json(run_jxa(&script)?)?;
        Ok(windows.into_iter().map(WindowInfo::from).collect())
    }

    fn get_focus(&self) -> Result<Option<FocusInfo>, OperatorError> {
        let script = r#"
const systemEvents = Application("System Events");
function safeAttr(target, name) {
  try {
    return target.attributes.byName(name).value();
  } catch (error) {
    return null;
  }
}
function safeCall(target, method) {
  try {
    return typeof target[method] === "function" ? target[method]() : null;
  } catch (error) {
    return null;
  }
}
function rectForElement(element) {
  try {
    const position = element.position();
    const size = element.size();
    return {
      x: Number(position[0]),
      y: Number(position[1]),
      width: Number(size[0]),
      height: Number(size[1])
    };
  } catch (error) {
    return null;
  }
}
const processes = systemEvents.applicationProcesses.whose({frontmost: true})();
if (processes.length === 0) {
  JSON.stringify(null);
} else {
  const process = processes[0];
  const focused = safeAttr(process, "AXFocusedUIElement");
  if (!focused) {
    JSON.stringify(null);
  } else {
    JSON.stringify({
      role: safeCall(focused, "role") || safeAttr(focused, "AXRole") || "AXUnknown",
      label: safeCall(focused, "description")
        || safeCall(focused, "name")
        || safeCall(focused, "title")
        || null,
      bounds: rectForElement(focused),
      app_name: process.name() || null
    });
  }
}
"#;

        let focus: Option<FocusRecord> = parse_jxa_json(run_jxa(script)?)?;
        Ok(focus.map(FocusInfo::from))
    }

    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError> {
        launch_with_open(bundle_id_or_name)
    }

    fn close_window(&self, id: WindowId) -> Result<(), OperatorError> {
        close_window_with_osascript(id)
    }

    fn minimize_window(&self, id: WindowId) -> Result<(), OperatorError> {
        minimize_window_with_osascript(id)
    }

    fn maximize_window(&self, id: WindowId) -> Result<(), OperatorError> {
        maximize_window_with_osascript(id)
    }

    fn quit_app(&self, app_name: &str) -> Result<(), OperatorError> {
        tell_application(app_name, "quit")
    }

    fn relaunch_app(&self, app_name: &str) -> Result<(), OperatorError> {
        tell_application(app_name, "quit")?;
        std::thread::sleep(std::time::Duration::from_millis(200));
        launch_with_open(app_name)
    }

    fn hide_app(&self, app_name: &str) -> Result<(), OperatorError> {
        set_application_visible(app_name, false)
    }

    fn unhide_app(&self, app_name: &str) -> Result<(), OperatorError> {
        set_application_visible(app_name, true)
    }

    fn focus_window(&self, id: WindowId) -> Result<(), OperatorError> {
        focus_window_with_osascript(id)
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

#[derive(Debug, Deserialize)]
struct FocusRecord {
    role: String,
    label: Option<String>,
    bounds: Option<Rect>,
    app_name: Option<String>,
}

impl From<FocusRecord> for FocusInfo {
    fn from(value: FocusRecord) -> Self {
        Self {
            role: value.role,
            label: value.label,
            bounds: value.bounds,
            app_name: value.app_name,
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
fn focus_window_with_osascript(id: WindowId) -> Result<(), OperatorError> {
    let script = format!(
        r#"
tell application "System Events"
  repeat with proc in application processes
    repeat with win in windows of proc
      if id of win is {window_id} then
        set frontmost of proc to true
        try
          perform action "AXRaise" of win
        end try
        return "{window_id}"
      end if
    end repeat
  end repeat
end tell
error "window {window_id} not found"
"#,
        window_id = id.0
    );

    run_osascript(&script).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn focus_window_with_osascript(_id: WindowId) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS window focus is unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn close_window_with_osascript(id: WindowId) -> Result<(), OperatorError> {
    press_window_button_with_osascript(id, "AXCloseButton")
}

#[cfg(not(target_os = "macos"))]
fn close_window_with_osascript(_id: WindowId) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS window chrome actions are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn minimize_window_with_osascript(id: WindowId) -> Result<(), OperatorError> {
    let script = format!(
        r#"
tell application "System Events"
  repeat with proc in application processes
    repeat with win in windows of proc
      if id of win is {window_id} then
        set value of attribute "AXMinimized" of win to true
        return "{window_id}"
      end if
    end repeat
  end repeat
end tell
error "window {window_id} not found"
"#,
        window_id = id.0
    );

    run_osascript(&script).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn minimize_window_with_osascript(_id: WindowId) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS window chrome actions are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn maximize_window_with_osascript(id: WindowId) -> Result<(), OperatorError> {
    press_window_button_with_osascript(id, "AXZoomButton")
}

#[cfg(not(target_os = "macos"))]
fn maximize_window_with_osascript(_id: WindowId) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS window chrome actions are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Result<String, OperatorError> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|error| OperatorError::Platform(format!("failed to invoke osascript: {error}")))?;

    command_output("osascript", output)
}

#[cfg(target_os = "macos")]
fn tell_application(app_name: &str, command: &str) -> Result<(), OperatorError> {
    let app_name = applescript_string_literal(app_name);
    let script = format!(
        r#"
tell application "{app_name}"
  {command}
end tell
"#
    );

    run_osascript(&script).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn tell_application(_app_name: &str, _command: &str) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS app lifecycle actions are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn set_application_visible(app_name: &str, visible: bool) -> Result<(), OperatorError> {
    let app_name = applescript_string_literal(app_name);
    let visible = if visible { "true" } else { "false" };
    let script = format!(
        r#"
tell application "System Events"
  set visible of application process "{app_name}" to {visible}
end tell
"#
    );

    run_osascript(&script).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn set_application_visible(_app_name: &str, _visible: bool) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS app lifecycle actions are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn press_window_button_with_osascript(
    id: WindowId,
    button_subrole: &str,
) -> Result<(), OperatorError> {
    let script = format!(
        r#"
tell application "System Events"
  repeat with proc in application processes
    repeat with win in windows of proc
      if id of win is {window_id} then
        perform action "AXPress" of (first button of win whose subrole is "{button_subrole}")
        return "{window_id}"
      end if
    end repeat
  end repeat
end tell
error "window {window_id} not found"
"#,
        window_id = id.0
    );

    run_osascript(&script).map(|_| ())
}

#[cfg(target_os = "macos")]
fn applescript_string_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
