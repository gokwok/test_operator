use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    ffi::CStr,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "macos")]
use cocoa::{
    base::{id, nil, NO},
    foundation::{NSAutoreleasePool, NSString},
};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
use operator_core::{AppInfo, AppListMode, FocusInfo, OperatorError, Rect, WindowId, WindowInfo};
use serde::Deserialize;

pub trait AppService: Send + Sync {
    fn list_apps(&self, mode: AppListMode) -> Result<Vec<AppInfo>, OperatorError>;
    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError>;
    fn list_frontmost_window_targets(&self) -> Result<Vec<WindowTarget>, OperatorError> {
        Ok(self
            .list_frontmost_windows()?
            .into_iter()
            .map(WindowTarget::from)
            .collect())
    }
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
    fn list_apps(&self, mode: AppListMode) -> Result<Vec<AppInfo>, OperatorError> {
        Ok(normalize_app_records(list_app_records_native(mode)?))
    }

    fn list_windows(&self, app: Option<&str>) -> Result<Vec<WindowInfo>, OperatorError> {
        Ok(list_window_records(app)?
            .into_iter()
            .map(WindowInfo::from)
            .collect())
    }

    fn list_frontmost_window_targets(&self) -> Result<Vec<WindowTarget>, OperatorError> {
        let windows: Vec<WindowRecord> = parse_jxa_json(run_jxa(frontmost_windows_script())?)?;
        Ok(windows.into_iter().map(WindowTarget::from).collect())
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
    path: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AppIdentityKey {
    BundleId(String),
    Path(String),
    Name(String),
}

fn normalize_app_records(apps: Vec<AppRecord>) -> Vec<AppInfo> {
    let mut normalized: Vec<_> = dedupe_app_records(apps)
        .into_iter()
        .map(AppInfo::from)
        .collect();
    normalized.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| right.is_running.cmp(&left.is_running))
            .then_with(|| left.pid.cmp(&right.pid))
            .then_with(|| left.bundle_id.cmp(&right.bundle_id))
    });
    normalized
}

fn dedupe_app_records(apps: Vec<AppRecord>) -> Vec<AppRecord> {
    let mut seen = HashSet::new();
    apps.into_iter()
        .filter(|app| seen.insert(app_identity_key(app)))
        .collect()
}

fn app_identity_key(app: &AppRecord) -> AppIdentityKey {
    if let Some(bundle_id) = &app.bundle_id {
        return AppIdentityKey::BundleId(bundle_id.to_lowercase());
    }

    if let Some(path) = &app.path {
        return AppIdentityKey::Path(path.to_lowercase());
    }

    AppIdentityKey::Name(app.name.to_lowercase())
}

#[cfg(target_os = "macos")]
fn list_app_records_native(mode: AppListMode) -> Result<Vec<AppRecord>, OperatorError> {
    let pool = unsafe { NSAutoreleasePool::new(nil) };
    let apps = unsafe {
        match mode {
            AppListMode::Running => list_running_app_records_native_inner(),
            AppListMode::All => list_all_app_records_native_inner(),
        }
    };
    unsafe { pool.drain() };
    apps
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
unsafe fn list_running_app_records_native_inner() -> Result<Vec<AppRecord>, OperatorError> {
    let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
    if workspace == nil {
        return Err(OperatorError::Platform(
            "failed to access shared NSWorkspace instance".into(),
        ));
    }

    let running_apps: id = msg_send![workspace, runningApplications];
    if running_apps == nil {
        return Ok(Vec::new());
    }

    let count: usize = msg_send![running_apps, count];
    let mut apps = Vec::with_capacity(count);

    for index in 0..count {
        let app: id = msg_send![running_apps, objectAtIndex: index];
        if app == nil {
            continue;
        }

        let is_terminated: bool = msg_send![app, isTerminated];
        if is_terminated != NO {
            continue;
        }

        let activation_policy: isize = msg_send![app, activationPolicy];
        if activation_policy == 2 {
            continue;
        }

        let Some(name) = nsstring_to_string(msg_send![app, localizedName]) else {
            continue;
        };
        let bundle_id = nsstring_to_string(msg_send![app, bundleIdentifier]);
        let bundle_path = nsurl_to_path(msg_send![app, bundleURL]);
        let pid: i32 = msg_send![app, processIdentifier];

        apps.push(AppRecord {
            bundle_id,
            name,
            pid: (pid > 0).then_some(pid as u32),
            is_running: true,
            path: bundle_path,
        });
    }

    Ok(apps)
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
unsafe fn list_all_app_records_native_inner() -> Result<Vec<AppRecord>, OperatorError> {
    let mut apps = list_running_app_records_native_inner()?;

    for root in application_search_roots() {
        collect_app_bundle_records(&root, &mut apps)?;
    }

    Ok(apps)
}

#[cfg(not(target_os = "macos"))]
fn list_app_records_native(_mode: AppListMode) -> Result<Vec<AppRecord>, OperatorError> {
    Err(OperatorError::Platform(
        "native app listing is only supported on macOS".into(),
    ))
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
unsafe fn nsstring_to_string(value: id) -> Option<String> {
    if value == nil {
        return None;
    }

    let utf8: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }

    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
unsafe fn nsurl_to_path(value: id) -> Option<String> {
    if value == nil {
        return None;
    }

    nsstring_to_string(msg_send![value, path])
}

#[cfg(target_os = "macos")]
fn application_search_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Library/CoreServices"),
        PathBuf::from("/System/Library/CoreServices/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    roots
}

#[cfg(target_os = "macos")]
fn collect_app_bundle_records(path: &Path, apps: &mut Vec<AppRecord>) -> Result<(), OperatorError> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(());
        }
        Err(error) => {
            return Err(OperatorError::Platform(format!(
                "failed to read application directory {}: {error}",
                path.display()
            )));
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                return Err(OperatorError::Platform(format!(
                    "failed to enumerate application directory {}: {error}",
                    path.display()
                )));
            }
        };
        let entry_path = entry.path();
        if is_app_bundle(&entry_path) {
            if let Some(record) = unsafe { bundle_record_from_path(&entry_path) } {
                apps.push(record);
            }
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                return Err(OperatorError::Platform(format!(
                    "failed to read file type for {}: {error}",
                    entry_path.display()
                )));
            }
        };
        if file_type.is_dir() {
            collect_app_bundle_records(&entry_path, apps)?;
        }
    }

    Ok(())
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
unsafe fn bundle_record_from_path(path: &Path) -> Option<AppRecord> {
    let bundle_path = path.to_str()?;
    let ns_path: id = NSString::alloc(nil).init_str(bundle_path);
    let bundle: id = msg_send![class!(NSBundle), bundleWithPath: ns_path];
    if bundle == nil {
        return None;
    }

    let bundle_id = nsstring_to_string(msg_send![bundle, bundleIdentifier]);
    let display_name_key: id = NSString::alloc(nil).init_str("CFBundleDisplayName");
    let bundle_name_key: id = NSString::alloc(nil).init_str("CFBundleName");
    let name = nsstring_to_string(msg_send![bundle, objectForInfoDictionaryKey: display_name_key])
        .or_else(|| {
            nsstring_to_string(msg_send![bundle, objectForInfoDictionaryKey: bundle_name_key])
        })
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })?;

    Some(AppRecord {
        bundle_id,
        name,
        pid: None,
        is_running: false,
        path: Some(bundle_path.to_string()),
    })
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

#[derive(Debug, Clone, PartialEq)]
pub struct WindowTarget {
    pub window: WindowInfo,
    pub native_id: Option<u64>,
    pub pid: Option<u32>,
    pub window_index: Option<usize>,
    pub ax_identifier: Option<String>,
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

impl From<WindowInfo> for WindowTarget {
    fn from(window: WindowInfo) -> Self {
        Self {
            native_id: (!is_synthetic_window_id(window.id)).then_some(window.id.0),
            window,
            pid: None,
            window_index: None,
            ax_identifier: None,
        }
    }
}

impl From<WindowRecord> for WindowTarget {
    fn from(value: WindowRecord) -> Self {
        Self {
            native_id: value.id,
            pid: value.pid,
            window_index: Some(value.window_index),
            ax_identifier: value.ax_identifier.clone(),
            window: WindowInfo::from(value),
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

    use super::{
        app_identity_key, dedupe_app_records, find_window_record, list_windows_script,
        normalize_app_records, AppIdentityKey, AppRecord, WindowRecord, SYNTHETIC_WINDOW_ID_MASK,
    };

    #[test]
    fn dedupe_app_records_collapses_same_bundle_identity() {
        let deduped = dedupe_app_records(vec![
            AppRecord {
                bundle_id: Some("com.apple.Safari".into()),
                name: "Safari".into(),
                pid: Some(2392),
                is_running: true,
                path: Some("/Applications/Safari.app".into()),
            },
            AppRecord {
                bundle_id: Some("com.apple.Safari".into()),
                name: "Safari".into(),
                pid: Some(2392),
                is_running: true,
                path: Some("/Applications/Safari.app".into()),
            },
            AppRecord {
                bundle_id: Some("com.apple.Safari".into()),
                name: "Safari".into(),
                pid: Some(2450),
                is_running: true,
                path: Some("/Applications/Safari Preview.app".into()),
            },
        ]);

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].pid, Some(2392));
    }

    #[test]
    fn app_identity_falls_back_to_bundle_path_then_name() {
        let app_with_path = AppRecord {
            bundle_id: None,
            name: "Calculator".into(),
            pid: None,
            is_running: false,
            path: Some("/Applications/Calculator.app".into()),
        };
        let app_without_path = AppRecord {
            bundle_id: None,
            name: "Calendar".into(),
            pid: None,
            is_running: false,
            path: None,
        };

        assert_eq!(
            app_identity_key(&app_with_path),
            AppIdentityKey::Path("/applications/calculator.app".into())
        );
        assert_eq!(
            app_identity_key(&app_without_path),
            AppIdentityKey::Name("calendar".into())
        );
    }

    #[test]
    fn normalize_app_records_sorts_running_entries_before_stopped_entries_for_same_name() {
        let normalized = normalize_app_records(vec![
            AppRecord {
                bundle_id: Some("com.apple.notes".into()),
                name: "Notes".into(),
                pid: Some(52),
                is_running: true,
                path: Some("/System/Applications/Notes.app".into()),
            },
            AppRecord {
                bundle_id: Some("com.apple.Safari".into()),
                name: "Safari".into(),
                pid: Some(91),
                is_running: true,
                path: Some("/Applications/Safari.app".into()),
            },
            AppRecord {
                bundle_id: Some("com.apple.notes.stopped".into()),
                name: "Notes".into(),
                pid: None,
                is_running: false,
                path: Some("/Applications/Notes Legacy.app".into()),
            },
        ]);

        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[0].name, "Notes");
        assert!(normalized[0].is_running);
        assert_eq!(normalized[0].pid, Some(52));
        assert_eq!(normalized[1].name, "Notes");
        assert!(!normalized[1].is_running);
        assert_eq!(normalized[1].pid, None);
        assert_eq!(normalized[2].name, "Safari");
        assert_eq!(normalized[2].pid, Some(91));
    }

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
