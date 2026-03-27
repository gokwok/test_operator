use std::collections::BTreeSet;

use hmdriver_rs::{CorrelatedWindow, CorrelatedWindowList, CurrentApp, WindowRect};
use operator_core::{ActionTargetSelector, AppInfo, OperatorError, Point, Rect, WindowInfo};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedActionTarget {
    pub(crate) app: Option<AppInfo>,
    pub(crate) window: Option<WindowInfo>,
}

pub(crate) fn normalize_apps(
    bundles: Vec<String>,
    current_app: Option<CurrentApp>,
) -> Vec<AppInfo> {
    let mut unique = BTreeSet::new();
    for bundle in bundles {
        insert_non_empty(&mut unique, bundle);
    }
    if let Some(current_app) = current_app {
        insert_non_empty(&mut unique, current_app.bundle_name);
    }

    unique
        .into_iter()
        .map(|bundle| AppInfo {
            bundle_id: Some(bundle.clone()),
            name: bundle,
            pid: None,
            is_running: true,
        })
        .collect()
}

pub(crate) fn normalize_windows(
    windows: CorrelatedWindowList,
    app_filter: Option<&str>,
) -> Vec<WindowInfo> {
    let mut normalized = normalized_window_candidates(windows, app_filter)
        .into_iter()
        .map(|candidate| candidate.window)
        .collect::<Vec<_>>();
    normalized.sort_by_key(|window| window.id.0);
    normalized
}

pub(crate) fn resolve_action_target(
    windows: CorrelatedWindowList,
    current_app: Option<CurrentApp>,
    selector: &ActionTargetSelector,
) -> Result<ResolvedActionTarget, OperatorError> {
    let candidates = normalized_window_candidates(windows, None);

    let resolved = match selector {
        ActionTargetSelector::App(bundle_id_or_name) => {
            let filter = normalize_match_text(bundle_id_or_name);
            let mut matches = candidates
                .iter()
                .filter(|candidate| {
                    candidate
                        .app
                        .as_ref()
                        .is_some_and(|app| app_matches(app, &filter))
                        || candidate
                            .window
                            .app_name
                            .as_deref()
                            .is_some_and(|name| normalize_match_text(name) == filter)
                })
                .cloned()
                .collect::<Vec<_>>();
            matches.sort_by_key(|candidate| (!candidate.window.is_focused, candidate.window.id.0));

            if let Some(candidate) = matches.into_iter().next() {
                ResolvedActionTarget {
                    app: candidate.app,
                    window: Some(candidate.window),
                }
            } else if current_app
                .as_ref()
                .is_some_and(|app| normalize_match_text(&app.bundle_name) == filter)
            {
                ResolvedActionTarget {
                    app: current_app.map(current_app_info),
                    window: None,
                }
            } else {
                return Err(OperatorError::Platform(format!(
                    "harmony.hdc action target app `{bundle_id_or_name}` was not found"
                )));
            }
        }
        ActionTargetSelector::Pid(pid) => {
            let Some(candidate) = candidates
                .into_iter()
                .find(|candidate| candidate.pid == Some(*pid))
            else {
                return Err(OperatorError::Platform(format!(
                    "harmony.hdc action target pid `{pid}` was not found"
                )));
            };
            ResolvedActionTarget {
                app: candidate.app,
                window: Some(candidate.window),
            }
        }
        ActionTargetSelector::WindowId(id) => {
            let Some(candidate) = candidates
                .into_iter()
                .find(|candidate| candidate.window.id == *id)
            else {
                return Err(OperatorError::Platform(format!(
                    "harmony.hdc action target window `{id}` was not found"
                )));
            };
            ResolvedActionTarget {
                app: candidate.app,
                window: Some(candidate.window),
            }
        }
        ActionTargetSelector::WindowTitle(title) => {
            let filter = normalize_match_text(title);
            let Some(candidate) = candidates.into_iter().find(|candidate| {
                candidate
                    .window
                    .title
                    .as_deref()
                    .is_some_and(|value| normalize_match_text(value) == filter)
            }) else {
                return Err(OperatorError::Platform(format!(
                    "harmony.hdc action target window `{title}` was not found"
                )));
            };
            ResolvedActionTarget {
                app: candidate.app,
                window: Some(candidate.window),
            }
        }
        ActionTargetSelector::WindowIndex(index) => {
            let Some(candidate) = candidates.get(*index).cloned() else {
                return Err(OperatorError::Platform(format!(
                    "harmony.hdc action target window index `{index}` was out of range"
                )));
            };
            ResolvedActionTarget {
                app: candidate.app,
                window: Some(candidate.window),
            }
        }
    };

    Ok(resolved)
}

pub(crate) fn target_anchor_point(target: &ResolvedActionTarget) -> Option<Point> {
    target.window.as_ref()?.bounds.map(rect_center)
}

fn normalized_window_candidates(
    windows: CorrelatedWindowList,
    app_filter: Option<&str>,
) -> Vec<NormalizedWindowCandidate> {
    let focused_window_id = windows.focused_window_id;
    let app_filter = app_filter.map(normalize_match_text);
    let mut normalized = windows
        .windows
        .into_iter()
        .filter_map(|entry| {
            normalize_window_candidate(entry, focused_window_id, app_filter.as_deref())
        })
        .collect::<Vec<_>>();
    normalized.sort_by_key(|candidate| candidate.window.id.0);
    normalized
}

fn normalize_window_candidate(
    entry: CorrelatedWindow,
    focused_window_id: Option<u32>,
    app_filter: Option<&str>,
) -> Option<NormalizedWindowCandidate> {
    let app_name = entry.mission.as_ref().and_then(|mission| {
        mission
            .app_name
            .clone()
            .or_else(|| mission.bundle_name.clone())
    });
    let bundle_id = entry
        .mission
        .as_ref()
        .and_then(|mission| mission.bundle_name.as_deref());

    if let Some(filter) = app_filter {
        let matches = app_name
            .as_deref()
            .is_some_and(|candidate| normalize_match_text(candidate) == filter)
            || bundle_id.is_some_and(|candidate| normalize_match_text(candidate) == filter);
        if !matches {
            return None;
        }
    }

    let pid = u32::try_from(entry.window.pid).ok();
    let window = WindowInfo {
        id: u64::from(entry.window.window_id).into(),
        title: non_empty(entry.window.name),
        app_name: app_name.clone(),
        bounds: rect_from_window(entry.window.rect),
        is_focused: focused_window_id == Some(entry.window.window_id),
        is_minimized: false,
    };

    Some(NormalizedWindowCandidate {
        app: app_info(bundle_id, app_name, pid),
        pid,
        window,
    })
}

fn rect_from_window(rect: WindowRect) -> Option<Rect> {
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    Some(Rect {
        x: f64::from(rect.x),
        y: f64::from(rect.y),
        width: f64::from(rect.width),
        height: f64::from(rect.height),
    })
}

fn insert_non_empty(set: &mut BTreeSet<String>, value: String) {
    let value = value.trim();
    if !value.is_empty() {
        set.insert(value.to_string());
    }
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone)]
struct NormalizedWindowCandidate {
    app: Option<AppInfo>,
    pid: Option<u32>,
    window: WindowInfo,
}

fn app_info(
    bundle_id: Option<&str>,
    app_name: Option<String>,
    pid: Option<u32>,
) -> Option<AppInfo> {
    let bundle_id = bundle_id.and_then(non_empty_str).map(ToOwned::to_owned);
    let name = app_name
        .clone()
        .or_else(|| bundle_id.clone())
        .or_else(|| pid.map(|value| format!("pid-{value}")))?;

    Some(AppInfo {
        bundle_id,
        name,
        pid,
        is_running: true,
    })
}

fn current_app_info(current_app: CurrentApp) -> AppInfo {
    AppInfo {
        bundle_id: Some(current_app.bundle_name.clone()),
        name: current_app.bundle_name,
        pid: None,
        is_running: true,
    }
}

fn app_matches(app: &AppInfo, filter: &str) -> bool {
    normalize_match_text(&app.name) == filter
        || app
            .bundle_id
            .as_deref()
            .is_some_and(|bundle_id| normalize_match_text(bundle_id) == filter)
}

fn rect_center(bounds: Rect) -> Point {
    Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    }
}

fn non_empty_str(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
