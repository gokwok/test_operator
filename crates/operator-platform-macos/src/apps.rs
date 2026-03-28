use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    process::Command,
};

use operator_core::{AppInfo, FocusInfo, OperatorError, Rect, WindowId, WindowInfo};
use serde::Deserialize;

pub trait AppService: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppInfo>, OperatorError>;
    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError>;
    fn list_frontmost_windows(&self) -> Result<Vec<WindowInfo>, OperatorError> {
        if let Some(app_name) = self.get_focus()?.and_then(|focus| focus.app_name) {
            return self.list_windows(Some(&app_name));
        }

        self.list_windows(None)
    }
    fn get_focus(&self) -> Result<Option<FocusInfo>, OperatorError>;
    fn launch_app(&self, bundle_id_or_name: &str) -> Result<(), OperatorError>;
    fn close_window(&self, id: WindowId) -> Result<(), OperatorError>;
    fn minimize_window(&self, id: WindowId) -> Result<(), OperatorError>;
    fn maximize_window(&self, id: WindowId) -> Result<(), OperatorError>;
    fn move_window(&self, id: WindowId, x: f64, y: f64) -> Result<Rect, OperatorError>;
    fn resize_window(&self, id: WindowId, width: f64, height: f64) -> Result<Rect, OperatorError>;
    fn set_window_bounds(&self, id: WindowId, bounds: Rect) -> Result<Rect, OperatorError>;
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
        Ok(list_window_records(app)?
            .into_iter()
            .map(WindowInfo::from)
            .collect())
    }

    fn list_frontmost_windows(&self) -> Result<Vec<WindowInfo>, OperatorError> {
        let windows: Vec<WindowRecord> = parse_jxa_json(run_jxa(frontmost_windows_script())?)?;
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
    JSON.stringify({
      role: "AXApplication",
      label: null,
      bounds: null,
      bundle_id: typeof process.bundleIdentifier === "function"
        ? process.bundleIdentifier()
        : null,
      app_name: process.name() || null
    });
  } else {
    JSON.stringify({
      role: safeCall(focused, "role") || safeAttr(focused, "AXRole") || "AXUnknown",
      label: safeCall(focused, "description")
        || safeCall(focused, "name")
        || safeCall(focused, "title")
        || null,
      bounds: rectForElement(focused),
      bundle_id: typeof process.bundleIdentifier === "function"
        ? process.bundleIdentifier()
        : null,
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

    fn move_window(&self, id: WindowId, x: f64, y: f64) -> Result<Rect, OperatorError> {
        let current = window_bounds_by_id(self, id)?;
        self.set_window_bounds(
            id,
            Rect {
                x,
                y,
                width: current.width,
                height: current.height,
            },
        )
    }

    fn resize_window(&self, id: WindowId, width: f64, height: f64) -> Result<Rect, OperatorError> {
        let current = window_bounds_by_id(self, id)?;
        self.set_window_bounds(
            id,
            Rect {
                x: current.x,
                y: current.y,
                width,
                height,
            },
        )
    }

    fn set_window_bounds(&self, id: WindowId, bounds: Rect) -> Result<Rect, OperatorError> {
        set_window_bounds_with_osascript(id, bounds)
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

fn list_windows_script(app_literal: &str) -> String {
    format!(
        r#"
const filter = {app_literal};
const systemEvents = Application("System Events");
function safeString(value) {{
  return value == null ? null : String(value);
}}
function safeCall(target, method) {{
  try {{
    return typeof target[method] === "function" ? target[method]() : null;
  }} catch (error) {{
    return null;
  }}
}}
function safeAttr(target, name) {{
  try {{
    return target.attributes.byName(name).value();
  }} catch (error) {{
    return null;
  }}
}}
function safeStringAttr(target, name) {{
  return safeString(safeAttr(target, name));
}}
function safeWindowId(window) {{
  try {{
    const value = safeCall(window, "id");
    if (value == null) {{
      return null;
    }}
    const number = Number(value);
    return Number.isFinite(number) ? number : null;
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
function safeProcessName(process) {{
  return safeString(safeCall(process, "name"));
}}
function safeProcessPid(process) {{
  const value = Number(safeCall(process, "unixId"));
  return Number.isFinite(value) ? value : null;
}}
function safeProcessFrontmost(process) {{
  return Boolean(safeCall(process, "frontmost"));
}}
function safeProcessWindows(process) {{
  const windows = safeCall(process, "windows");
  return windows == null ? [] : windows;
}}
const processes = filter
  ? systemEvents.applicationProcesses.whose({{name: filter}})()
  : systemEvents.applicationProcesses();
let windows = [];
for (const process of processes) {{
  const appName = safeProcessName(process);
  const pid = safeProcessPid(process);
  const isFrontmost = safeProcessFrontmost(process);
  const processWindows = safeProcessWindows(process);
  for (let index = 0; index < processWindows.length; index += 1) {{
    const window = processWindows[index];
    const isMain = safeAttr(window, "AXMain");
    const isFocused = safeAttr(window, "AXFocused");
    windows.push({{
      id: safeWindowId(window),
      pid: pid,
      window_index: index,
      ax_identifier: safeStringAttr(window, "AXIdentifier"),
      title: safeString(safeCall(window, "name")),
      app_name: appName,
      bounds: rectForElement(window),
      is_focused: Boolean(isFrontmost && (isMain || isFocused)),
      is_minimized: Boolean(safeAttr(window, "AXMinimized"))
    }});
  }}
}}
JSON.stringify(windows);
"#
    )
}

fn frontmost_windows_script() -> &'static str {
    r#"
const systemEvents = Application("System Events");
function safeString(value) {
  return value == null ? null : String(value);
}
function safeCall(target, method) {
  try {
    return typeof target[method] === "function" ? target[method]() : null;
  } catch (error) {
    return null;
  }
}
function safeAttr(target, name) {
  try {
    return target.attributes.byName(name).value();
  } catch (error) {
    return null;
  }
}
function safeStringAttr(target, name) {
  return safeString(safeAttr(target, name));
}
function safeWindowId(window) {
  try {
    const value = safeCall(window, "id");
    if (value == null) {
      return null;
    }
    const number = Number(value);
    return Number.isFinite(number) ? number : null;
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
function safeProcessName(process) {
  return safeString(safeCall(process, "name"));
}
function safeProcessPid(process) {
  const value = Number(safeCall(process, "unixId"));
  return Number.isFinite(value) ? value : null;
}
function safeProcessFrontmost(process) {
  return Boolean(safeCall(process, "frontmost"));
}
function safeProcessWindows(process) {
  const windows = safeCall(process, "windows");
  return windows == null ? [] : windows;
}
const processes = systemEvents.applicationProcesses.whose({frontmost: true})();
if (!processes.length) {
  JSON.stringify([]);
} else {
  const process = processes[0];
  const appName = safeProcessName(process);
  const pid = safeProcessPid(process);
  const isFrontmost = safeProcessFrontmost(process);
  const processWindows = safeProcessWindows(process);
  let windows = [];
  for (let index = 0; index < processWindows.length; index += 1) {
    const window = processWindows[index];
    const isMain = safeAttr(window, "AXMain");
    const isFocused = safeAttr(window, "AXFocused");
    windows.push({
      id: safeWindowId(window),
      pid: pid,
      window_index: index,
      ax_identifier: safeStringAttr(window, "AXIdentifier"),
      title: safeString(safeCall(window, "name")),
      app_name: appName,
      bounds: rectForElement(window),
      is_focused: Boolean(isFrontmost && (isMain || isFocused)),
      is_minimized: Boolean(safeAttr(window, "AXMinimized"))
    });
  }
  JSON.stringify(windows);
}
"#
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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WindowRecord {
    pub(crate) id: Option<u64>,
    pub(crate) pid: Option<u32>,
    pub(crate) window_index: usize,
    pub(crate) ax_identifier: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) app_name: Option<String>,
    pub(crate) bounds: Option<Rect>,
    pub(crate) is_focused: bool,
    pub(crate) is_minimized: bool,
}

impl WindowRecord {
    pub(crate) fn public_id(&self) -> WindowId {
        self.id
            .map(WindowId::from)
            .unwrap_or_else(|| synthetic_window_id(self))
    }
}

impl From<WindowRecord> for WindowInfo {
    fn from(value: WindowRecord) -> Self {
        Self {
            id: value.public_id(),
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
    bundle_id: Option<String>,
    app_name: Option<String>,
}

impl From<FocusRecord> for FocusInfo {
    fn from(value: FocusRecord) -> Self {
        Self {
            role: value.role,
            label: value.label,
            bounds: value.bounds,
            bundle_id: value.bundle_id,
            app_name: value.app_name,
        }
    }
}

pub(crate) const SYNTHETIC_WINDOW_ID_MASK: u64 = 1 << 63;

fn synthetic_window_id(window: &WindowRecord) -> WindowId {
    // Some macOS apps expose enough AX metadata to observe and verify windows but do not
    // surface a native `window.id()` through System Events. Derive a stable per-process id so
    // verification can still correlate the same window across repeated `list_windows` calls.
    let mut hasher = DefaultHasher::new();
    window.pid.hash(&mut hasher);
    window.app_name.hash(&mut hasher);
    if let Some(identifier) = window
        .ax_identifier
        .as_deref()
        .filter(|identifier| !identifier.is_empty())
    {
        identifier.hash(&mut hasher);
    } else {
        window.title.hash(&mut hasher);
        window.window_index.hash(&mut hasher);
    }

    let mut value = hasher.finish() & !SYNTHETIC_WINDOW_ID_MASK;
    if value == 0 {
        value = 1;
    }
    WindowId(value | SYNTHETIC_WINDOW_ID_MASK)
}

pub(crate) fn is_synthetic_window_id(id: WindowId) -> bool {
    id.0 & SYNTHETIC_WINDOW_ID_MASK != 0
}

pub(crate) fn list_window_records(app: Option<&str>) -> Result<Vec<WindowRecord>, OperatorError> {
    let app_literal = serde_json::to_string(&app).map_err(|error| {
        OperatorError::Platform(format!("failed to encode app filter: {error}"))
    })?;
    let script = list_windows_script(&app_literal);

    parse_jxa_json(run_jxa(&script)?)
}

pub(crate) fn resolve_window_record(id: WindowId) -> Result<WindowRecord, OperatorError> {
    let windows = list_window_records(None)?;
    find_window_record(&windows, id)
        .cloned()
        .ok_or_else(|| OperatorError::Platform(format!("window {id} not found")))
}

fn find_window_record(windows: &[WindowRecord], id: WindowId) -> Option<&WindowRecord> {
    windows.iter().find(|window| window.public_id() == id)
}

fn parse_jxa_json<T>(json: String) -> Result<T, OperatorError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(&json).map_err(|error| {
        OperatorError::Platform(format!("failed to decode macOS command output: {error}"))
    })
}

fn window_bounds_by_id<A: AppService + ?Sized>(
    app_service: &A,
    id: WindowId,
) -> Result<Rect, OperatorError> {
    let windows = app_service.list_windows(None)?;
    let window = windows
        .into_iter()
        .find(|window| window.id == id)
        .ok_or_else(|| OperatorError::Platform(format!("window {id} not found")))?;
    window.bounds.ok_or_else(|| {
        OperatorError::Platform(format!("window {id} has no bounds available on macOS"))
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

    command_output("open", output)?;
    activate_application(bundle_id_or_name)?;
    std::thread::sleep(std::time::Duration::from_millis(200));
    Ok(())
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
fn set_window_bounds_with_osascript(id: WindowId, bounds: Rect) -> Result<Rect, OperatorError> {
    let right = bounds.x + bounds.width;
    let bottom = bounds.y + bounds.height;
    let script = format!(
        r#"
tell application "System Events"
  repeat with proc in application processes
    repeat with win in windows of proc
      if id of win is {window_id} then
        set bounds of win to {{{left}, {top}, {right}, {bottom}}}
        set updatedBounds to bounds of win
        return (item 1 of updatedBounds as string) & "," & (item 2 of updatedBounds as string) & "," & (item 3 of updatedBounds as string) & "," & (item 4 of updatedBounds as string)
      end if
    end repeat
  end repeat
end tell
error "window {window_id} not found"
"#,
        window_id = id.0,
        left = bounds.x,
        top = bounds.y,
        right = right,
        bottom = bottom
    );

    parse_window_bounds_csv(&run_osascript(&script)?)
}

#[cfg(not(target_os = "macos"))]
fn set_window_bounds_with_osascript(_id: WindowId, _bounds: Rect) -> Result<Rect, OperatorError> {
    Err(OperatorError::Platform(
        "macOS window geometry actions are unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(target_os = "macos")]
fn parse_window_bounds_csv(csv: &str) -> Result<Rect, OperatorError> {
    let values = csv
        .split(',')
        .map(str::trim)
        .map(|value| {
            value.parse::<f64>().map_err(|error| {
                OperatorError::Platform(format!(
                    "failed to parse macOS window bounds component {value:?}: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if values.len() != 4 {
        return Err(OperatorError::Platform(format!(
            "failed to parse macOS window bounds: {csv}"
        )));
    }

    Ok(Rect {
        x: values[0],
        y: values[1],
        width: values[2] - values[0],
        height: values[3] - values[1],
    })
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
fn activate_application(bundle_id_or_name: &str) -> Result<(), OperatorError> {
    let script = if bundle_id_or_name.contains('.') {
        format!(
            r#"tell application id "{}" to activate"#,
            applescript_string_literal(bundle_id_or_name)
        )
    } else {
        format!(
            r#"tell application "{}" to activate"#,
            applescript_string_literal(bundle_id_or_name)
        )
    };

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

#[cfg(test)]
mod tests {
    use operator_core::{WindowId, WindowInfo};

    use super::{find_window_record, list_windows_script, WindowRecord, SYNTHETIC_WINDOW_ID_MASK};

    #[test]
    fn missing_native_window_id_uses_stable_synthetic_id() {
        let record = WindowRecord {
            id: None,
            pid: Some(42),
            window_index: 0,
            ax_identifier: Some("main".into()),
            title: Some("计算器".into()),
            app_name: Some("Calculator".into()),
            bounds: None,
            is_focused: true,
            is_minimized: false,
        };

        let first = WindowInfo::from(record.clone());
        let second = WindowInfo::from(record);

        assert_eq!(first.id, second.id);
        assert_ne!(first.id.0 & SYNTHETIC_WINDOW_ID_MASK, 0);
    }

    #[test]
    fn synthetic_window_id_changes_when_window_slot_changes_without_identifier() {
        let first = WindowInfo::from(WindowRecord {
            id: None,
            pid: Some(42),
            window_index: 0,
            ax_identifier: None,
            title: Some("Untitled".into()),
            app_name: Some("Notes".into()),
            bounds: None,
            is_focused: false,
            is_minimized: false,
        });
        let second = WindowInfo::from(WindowRecord {
            id: None,
            pid: Some(42),
            window_index: 1,
            ax_identifier: None,
            title: Some("Untitled".into()),
            app_name: Some("Notes".into()),
            bounds: None,
            is_focused: false,
            is_minimized: false,
        });

        assert_ne!(first.id, second.id);
    }

    #[test]
    fn find_window_record_matches_native_and_synthetic_public_ids() {
        let native = WindowRecord {
            id: Some(77),
            pid: Some(10),
            window_index: 0,
            ax_identifier: None,
            title: Some("Native".into()),
            app_name: Some("Preview".into()),
            bounds: None,
            is_focused: false,
            is_minimized: false,
        };
        let synthetic = WindowRecord {
            id: None,
            pid: Some(11),
            window_index: 1,
            ax_identifier: Some("editor".into()),
            title: Some("Scratch".into()),
            app_name: Some("Codex".into()),
            bounds: None,
            is_focused: false,
            is_minimized: false,
        };
        let windows = vec![native.clone(), synthetic.clone()];

        assert_eq!(
            find_window_record(&windows, WindowId::from(77))
                .unwrap()
                .title,
            native.title
        );
        assert_eq!(
            find_window_record(&windows, synthetic.public_id())
                .unwrap()
                .app_name,
            synthetic.app_name
        );
    }

    #[test]
    fn list_windows_script_skips_processes_with_inaccessible_window_lists() {
        let script = list_windows_script("null");

        assert!(script.contains("function safeProcessWindows(process)"));
        assert!(script.contains("const processWindows = safeProcessWindows(process);"));
        assert!(!script.contains("const processWindows = process.windows();"));
    }

    #[test]
    fn list_windows_script_wraps_process_metadata_accesses() {
        let script = list_windows_script("null");

        assert!(script.contains("const appName = safeProcessName(process);"));
        assert!(script.contains("const pid = safeProcessPid(process);"));
        assert!(script.contains("const isFrontmost = safeProcessFrontmost(process);"));
        assert!(script.contains("title: safeString(safeCall(window, \"name\"))"));
    }
}
