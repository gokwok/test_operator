use std::{collections::HashMap, process::Command};

use operator_core::{
    ElementId, ElementSource, OperatorError, Rect, Surface, SurfaceKind, UiElement,
};
use serde::Deserialize;

use crate::apps::{is_synthetic_window_id, resolve_window_record, WindowRecord};

pub trait TreeInspector: Send + Sync {
    fn inspect(&self, surface: &Surface) -> Result<InspectResult, OperatorError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectResult {
    pub elements: HashMap<ElementId, UiElement>,
    pub root_ids: Vec<ElementId>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTreeInspector;

impl TreeInspector for SystemTreeInspector {
    fn inspect(&self, surface: &Surface) -> Result<InspectResult, OperatorError> {
        let output = run_jxa(&script_for_surface(surface)?)?;
        let roots = parse_inspection_nodes(&output)?;

        let mut elements = HashMap::new();
        let mut root_ids = Vec::new();
        for (index, node) in roots.into_iter().enumerate() {
            let id = flatten_node(node, &format!("ax-{index}"), &mut elements);
            root_ids.push(id);
        }

        let result = InspectResult { elements, root_ids };
        if let SurfaceKind::Region { rect } = &surface.kind {
            return Ok(filter_to_region(result, *rect));
        }

        Ok(result)
    }
}

#[derive(Debug, Deserialize)]
struct InspectNode {
    role: String,
    label: Option<String>,
    value: Option<String>,
    bounds: Option<Rect>,
    enabled: Option<bool>,
    #[serde(default)]
    children: Vec<InspectNode>,
}

fn flatten_node(
    node: InspectNode,
    path: &str,
    elements: &mut HashMap<ElementId, UiElement>,
) -> ElementId {
    let id = ElementId(path.into());
    let mut child_ids = Vec::new();
    for (index, child) in node.children.into_iter().enumerate() {
        child_ids.push(flatten_node(child, &format!("{path}-{index}"), elements));
    }

    elements.insert(
        id.clone(),
        UiElement {
            id: id.clone(),
            role: node.role,
            label: empty_string_as_none(node.label),
            value: empty_string_as_none(node.value),
            bounds: node.bounds,
            enabled: node.enabled,
            children: child_ids,
            confidence: Some(1.0),
            source: ElementSource::Native,
        },
    );

    id
}

fn empty_string_as_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_inspection_nodes(json: &str) -> Result<Vec<InspectNode>, OperatorError> {
    serde_json::from_str::<Vec<InspectNode>>(json).map_err(|error| {
        OperatorError::Platform(format!("failed to decode macOS inspection output: {error}"))
    })
}

fn script_for_surface(surface: &Surface) -> Result<String, OperatorError> {
    const TEMPLATE: &str = r#"
const systemEvents = Application("System Events");

function optional(callback) {
  try {
    return callback();
  } catch (_) {
    return null;
  }
}

function toText(value) {
  if (value === null || value === undefined) {
    return null;
  }
  return String(value);
}

function boundsFor(element) {
  const position = optional(() => element.position());
  const size = optional(() => element.size());
  if (!position || !size || position.length < 2 || size.length < 2) {
    return null;
  }

  return {
    x: Number(position[0]),
    y: Number(position[1]),
    width: Number(size[0]),
    height: Number(size[1]),
  };
}

function serialize(element, depth) {
  const role = optional(() => element.role()) || "AXUnknown";
  const children = depth >= 5
    ? []
    : (optional(() => element.uiElements()) || []).slice(0, 50).map((child) => serialize(child, depth + 1));

  return {
    role,
    label: toText(optional(() => element.name())),
    value: toText(optional(() => element.value())),
    bounds: boundsFor(element),
    enabled: optional(() => Boolean(element.enabled())),
    children,
  };
}

const roots = __ROOT_QUERY__;
JSON.stringify(roots.map((root) => serialize(root, 0)));
"#;

    Ok(TEMPLATE.replace("__ROOT_QUERY__", &root_query(surface)?))
}

fn root_query(surface: &Surface) -> Result<String, OperatorError> {
    match &surface.kind {
        SurfaceKind::Window { id } if is_synthetic_window_id(*id) => {
            synthetic_window_root_query(*id, &resolve_window_record(*id)?)
        }
        SurfaceKind::Window { id } => Ok(format!(
            r#"(() => {{
  const matches = [];
  const processes = systemEvents.applicationProcesses();
  for (const process of processes) {{
    const windows = optional(() => process.windows()) || [];
    for (const window of windows) {{
      if (optional(() => Number(window.id())) === {window_id}) {{
        matches.push(window);
      }}
    }}
  }}

  if (!matches.length) {{
    throw new Error("window {window_id} not found");
  }}

  return matches.slice(0, 1);
}})()"#,
            window_id = id.0,
        )),
        SurfaceKind::Frontmost => Ok(frontmost_root_query()),
        SurfaceKind::Fullscreen { .. } | SurfaceKind::Region { .. } => Ok(all_window_roots_query()),
    }
}

fn frontmost_root_query() -> String {
    r#"(() => {
  const processes = systemEvents.applicationProcesses.whose({frontmost: true})();
  if (!processes.length) {
    throw new Error("no frontmost application process");
  }

  const process = processes[0];
  const windows = optional(() => process.windows()) || [];
  return windows.length ? windows.slice(0, 1) : [process];
})()"#
        .into()
}

fn all_window_roots_query() -> String {
    r#"(() => {
  const roots = [];
  const processes = systemEvents.applicationProcesses();

  for (const process of processes) {
    const windows = optional(() => process.windows()) || [];
    for (const window of windows) {
      if (optional(() => Boolean(window.attributes.byName("AXMinimized").value()))) {
        continue;
      }
      roots.push(window);
    }
  }

  if (roots.length) {
    return roots;
  }

  const frontmost = systemEvents.applicationProcesses.whose({frontmost: true})();
  if (!frontmost.length) {
    throw new Error("no application process with inspectable windows");
  }

  const process = frontmost[0];
  const windows = optional(() => process.windows()) || [];
  return windows.length ? windows.slice(0, 1) : [process];
})()"#
        .into()
}

fn filter_to_region(result: InspectResult, region: Rect) -> InspectResult {
    let mut filtered = HashMap::new();
    let root_ids = result
        .root_ids
        .iter()
        .filter_map(|id| filter_element_to_region(&result.elements, &mut filtered, id, region))
        .collect();

    InspectResult {
        elements: filtered,
        root_ids,
    }
}

fn filter_element_to_region(
    elements: &HashMap<ElementId, UiElement>,
    filtered: &mut HashMap<ElementId, UiElement>,
    id: &ElementId,
    region: Rect,
) -> Option<ElementId> {
    let element = elements.get(id)?;
    let children = element
        .children
        .iter()
        .filter_map(|child| filter_element_to_region(elements, filtered, child, region))
        .collect::<Vec<_>>();
    let keep = element
        .bounds
        .is_some_and(|bounds| rects_intersect(bounds, region))
        || !children.is_empty();
    if !keep {
        return None;
    }

    let mut element = element.clone();
    element.children = children;
    filtered.insert(id.clone(), element);
    Some(id.clone())
}

fn rects_intersect(lhs: Rect, rhs: Rect) -> bool {
    let left = lhs.x.max(rhs.x);
    let top = lhs.y.max(rhs.y);
    let right = (lhs.x + lhs.width).min(rhs.x + rhs.width);
    let bottom = (lhs.y + lhs.height).min(rhs.y + rhs.height);

    left < right && top < bottom
}

fn synthetic_window_root_query(
    id: operator_core::WindowId,
    window: &WindowRecord,
) -> Result<String, OperatorError> {
    let pid = window
        .pid
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "null".into());
    let app_name = serde_json::to_string(&window.app_name).map_err(|error| {
        OperatorError::Platform(format!("failed to encode macOS app name: {error}"))
    })?;
    let title = serde_json::to_string(&window.title).map_err(|error| {
        OperatorError::Platform(format!("failed to encode macOS window title: {error}"))
    })?;
    let ax_identifier = serde_json::to_string(&window.ax_identifier).map_err(|error| {
        OperatorError::Platform(format!(
            "failed to encode macOS window AX identifier: {error}"
        ))
    })?;

    Ok(format!(
        r#"(() => {{
  const expectedPid = {pid};
  const expectedAppName = {app_name};
  const expectedTitle = {title};
  const expectedIdentifier = {ax_identifier};
  const expectedIndex = {window_index};
  const processes = systemEvents.applicationProcesses();

  for (const process of processes) {{
    if (expectedPid !== null && optional(() => Number(process.unixId())) !== expectedPid) {{
      continue;
    }}
    if (
      expectedPid === null &&
      expectedAppName !== null &&
      toText(optional(() => process.name())) !== expectedAppName
    ) {{
      continue;
    }}

    const windows = optional(() => process.windows()) || [];
    if (expectedIdentifier !== null) {{
      for (const window of windows) {{
        if (
          toText(optional(() => window.attributes.byName("AXIdentifier").value())) ===
          expectedIdentifier
        ) {{
          return [window];
        }}
      }}
    }}

    const window = windows[expectedIndex];
    if (!window) {{
      continue;
    }}

    if (expectedTitle === null || toText(optional(() => window.name())) === expectedTitle) {{
      return [window];
    }}
  }}

  throw new Error("window {window_id} not found");
}})()"#,
        pid = pid,
        app_name = app_name,
        title = title,
        ax_identifier = ax_identifier,
        window_index = window.window_index,
        window_id = id.0,
    ))
}

#[cfg(target_os = "macos")]
fn run_jxa(script: &str) -> Result<String, OperatorError> {
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .map_err(|error| OperatorError::Platform(format!("failed to invoke osascript: {error}")))?;

    command_output("osascript", output)
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

#[cfg(not(target_os = "macos"))]
fn run_jxa(_script: &str) -> Result<String, OperatorError> {
    Err(OperatorError::Platform(
        "macOS tree inspection is unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use operator_core::{ElementId, ElementSource, UiElement, WindowId};

    use super::{
        filter_to_region, root_query, synthetic_window_root_query, InspectResult, Surface,
        SurfaceKind, WindowRecord,
    };
    use operator_core::Rect;

    #[test]
    fn synthetic_window_query_matches_pid_identifier_and_index() {
        let query = synthetic_window_root_query(
            WindowId((1 << 63) | 42),
            &WindowRecord {
                id: None,
                pid: Some(512),
                window_index: 3,
                ax_identifier: Some("workspace.editor".into()),
                title: Some("main.rs".into()),
                app_name: Some("Codex".into()),
                bounds: None,
                is_focused: false,
                is_minimized: false,
            },
        )
        .unwrap();

        assert!(query.contains("const expectedPid = 512;"));
        assert!(query.contains("const expectedIdentifier = \"workspace.editor\";"));
        assert!(query.contains("const expectedIndex = 3;"));
        assert!(query.contains("window.attributes.byName(\"AXIdentifier\").value()"));
    }

    #[test]
    fn fullscreen_query_enumerates_non_minimized_windows() {
        let query = root_query(&Surface {
            kind: SurfaceKind::Fullscreen {
                display_id: Some(2),
            },
        })
        .unwrap();

        assert!(query.contains("const roots = [];"));
        assert!(query.contains("systemEvents.applicationProcesses();"));
        assert!(query.contains("AXMinimized"));
        assert!(query.contains("return roots;"));
    }

    #[test]
    fn region_filter_keeps_only_intersecting_subtrees() {
        let result = filter_to_region(
            InspectResult {
                elements: HashMap::from([
                    (
                        ElementId("ax-0".into()),
                        UiElement {
                            id: ElementId("ax-0".into()),
                            role: "AXWindow".into(),
                            label: Some("Editor".into()),
                            value: None,
                            bounds: Some(Rect {
                                x: 0.0,
                                y: 0.0,
                                width: 400.0,
                                height: 300.0,
                            }),
                            enabled: Some(true),
                            children: vec![ElementId("ax-0-0".into()), ElementId("ax-0-1".into())],
                            confidence: Some(1.0),
                            source: ElementSource::Native,
                        },
                    ),
                    (
                        ElementId("ax-0-0".into()),
                        UiElement {
                            id: ElementId("ax-0-0".into()),
                            role: "AXButton".into(),
                            label: Some("Inside".into()),
                            value: None,
                            bounds: Some(Rect {
                                x: 120.0,
                                y: 80.0,
                                width: 40.0,
                                height: 30.0,
                            }),
                            enabled: Some(true),
                            children: vec![],
                            confidence: Some(1.0),
                            source: ElementSource::Native,
                        },
                    ),
                    (
                        ElementId("ax-0-1".into()),
                        UiElement {
                            id: ElementId("ax-0-1".into()),
                            role: "AXButton".into(),
                            label: Some("Outside".into()),
                            value: None,
                            bounds: Some(Rect {
                                x: 320.0,
                                y: 240.0,
                                width: 40.0,
                                height: 30.0,
                            }),
                            enabled: Some(true),
                            children: vec![],
                            confidence: Some(1.0),
                            source: ElementSource::Native,
                        },
                    ),
                    (
                        ElementId("ax-1".into()),
                        UiElement {
                            id: ElementId("ax-1".into()),
                            role: "AXWindow".into(),
                            label: Some("Terminal".into()),
                            value: None,
                            bounds: Some(Rect {
                                x: 600.0,
                                y: 50.0,
                                width: 300.0,
                                height: 200.0,
                            }),
                            enabled: Some(true),
                            children: vec![ElementId("ax-1-0".into())],
                            confidence: Some(1.0),
                            source: ElementSource::Native,
                        },
                    ),
                    (
                        ElementId("ax-1-0".into()),
                        UiElement {
                            id: ElementId("ax-1-0".into()),
                            role: "AXStaticText".into(),
                            label: Some("Far away".into()),
                            value: None,
                            bounds: Some(Rect {
                                x: 620.0,
                                y: 70.0,
                                width: 80.0,
                                height: 20.0,
                            }),
                            enabled: Some(true),
                            children: vec![],
                            confidence: Some(1.0),
                            source: ElementSource::Native,
                        },
                    ),
                ]),
                root_ids: vec![ElementId("ax-0".into()), ElementId("ax-1".into())],
            },
            Rect {
                x: 100.0,
                y: 60.0,
                width: 120.0,
                height: 80.0,
            },
        );

        assert_eq!(result.root_ids, vec![ElementId("ax-0".into())]);
        assert!(result.elements.contains_key(&ElementId("ax-0".into())));
        assert!(result.elements.contains_key(&ElementId("ax-0-0".into())));
        assert!(!result.elements.contains_key(&ElementId("ax-0-1".into())));
        assert!(!result.elements.contains_key(&ElementId("ax-1".into())));
    }
}
