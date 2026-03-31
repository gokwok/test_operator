use std::collections::BTreeMap;

use hmdriver_rs::{CorrelatedWindow, CorrelatedWindowList, CurrentApp, WindowRect};
use operator_core::{ActionTargetSelector, AppInfo, OperatorError, Point, Rect, WindowInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedActionTarget {
    pub(crate) app: Option<AppInfo>,
    pub(crate) window: Option<WindowInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InstalledHarmonyApp {
    pub(crate) bundle_id: String,
    pub(crate) name: String,
}

pub(crate) fn normalize_all_apps(
    installed_apps: Vec<InstalledHarmonyApp>,
    windows: CorrelatedWindowList,
    labels: &BTreeMap<String, String>,
) -> Vec<AppInfo> {
    let mut apps = running_app_records_from_windows(windows, labels);
    apps.extend(installed_app_records(installed_apps));
    normalize_app_records(apps)
}

pub(crate) fn normalize_running_apps(
    windows: CorrelatedWindowList,
    labels: &BTreeMap<String, String>,
) -> Vec<AppInfo> {
    normalize_app_records(running_app_records_from_windows(windows, labels))
}

pub(crate) fn normalize_windows(
    windows: CorrelatedWindowList,
    labels: &BTreeMap<String, String>,
    app_filter: Option<&str>,
) -> Vec<WindowInfo> {
    let mut normalized = normalized_window_candidates(windows, labels, app_filter)
        .into_iter()
        .map(|candidate| candidate.window)
        .collect::<Vec<_>>();
    normalized.sort_by_key(|window| window.id.0);
    normalized
}

pub(crate) fn resolve_action_target(
    windows: CorrelatedWindowList,
    current_app: Option<CurrentApp>,
    labels: &BTreeMap<String, String>,
    selector: &ActionTargetSelector,
) -> Result<ResolvedActionTarget, OperatorError> {
    let candidates = normalized_window_candidates(windows, labels, None);

    let resolved = match selector {
        ActionTargetSelector::App(bundle_id_or_name) => {
            let filter = normalize_match_text(bundle_id_or_name);
            let mut matches = candidates
                .iter()
                .filter(|candidate| candidate_matches_app_selector(candidate, &filter, labels))
                .cloned()
                .collect::<Vec<_>>();
            matches.sort_by_key(|candidate| (!candidate.window.is_focused, candidate.window.id.0));

            if let Some(candidate) = matches.into_iter().next() {
                ResolvedActionTarget {
                    app: candidate.app,
                    window: Some(candidate.window),
                }
            } else if current_app.as_ref().is_some_and(|app| {
                normalize_match_text(&app.bundle_name) == filter
                    || labels
                        .get(&app.bundle_name)
                        .is_some_and(|label| normalize_match_text(label) == filter)
            }) {
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
    labels: &BTreeMap<String, String>,
    app_filter: Option<&str>,
) -> Vec<NormalizedWindowCandidate> {
    let focused_window_id = windows.focused_window_id;
    let app_filter = app_filter.map(normalize_match_text);
    let mut normalized = windows
        .windows
        .into_iter()
        .filter_map(|entry| {
            normalize_window_candidate(entry, focused_window_id, labels, app_filter.as_deref())
        })
        .collect::<Vec<_>>();
    normalized.sort_by_key(|candidate| candidate.window.id.0);
    normalized
}

fn normalize_window_candidate(
    entry: CorrelatedWindow,
    focused_window_id: Option<u32>,
    labels: &BTreeMap<String, String>,
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
            || bundle_id.is_some_and(|candidate| normalize_match_text(candidate) == filter)
            || bundle_id
                .and_then(|candidate| labels.get(candidate))
                .is_some_and(|label| normalize_match_text(label) == filter);
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppRecord {
    bundle_id: Option<String>,
    name: String,
    pid: Option<u32>,
    is_running: bool,
}

fn running_app_records_from_windows(
    windows: CorrelatedWindowList,
    labels: &BTreeMap<String, String>,
) -> Vec<AppRecord> {
    normalized_window_candidates(windows, labels, None)
        .into_iter()
        .filter_map(|candidate| candidate.app)
        .filter(is_listable_app_info)
        .map(|app| {
            let name = resolved_running_app_name(&app, labels);
            AppRecord {
                bundle_id: app.bundle_id,
                name,
                pid: app.pid,
                is_running: true,
            }
        })
        .collect()
}

fn installed_app_records(apps: Vec<InstalledHarmonyApp>) -> Vec<AppRecord> {
    apps.into_iter()
        .filter_map(|bundle| {
            let bundle_id = bundle.bundle_id.trim();
            let name = bundle.name.trim();
            if bundle_id.is_empty() || name.is_empty() {
                return None;
            }

            Some(AppRecord {
                bundle_id: Some(bundle_id.to_string()),
                name: name.to_string(),
                pid: None,
                is_running: false,
            })
        })
        .collect()
}

fn normalize_app_records(apps: Vec<AppRecord>) -> Vec<AppInfo> {
    let mut deduped = BTreeMap::new();

    for app in apps {
        let key = app_identity_key(&app);
        deduped
            .entry(key)
            .and_modify(|existing| merge_app_record(existing, &app))
            .or_insert(app);
    }

    let mut normalized = deduped
        .into_values()
        .map(|record| AppInfo {
            bundle_id: record.bundle_id,
            name: record.name,
            pid: record.pid,
            is_running: record.is_running,
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then_with(|| right.is_running.cmp(&left.is_running))
            .then_with(|| left.pid.cmp(&right.pid))
            .then_with(|| left.bundle_id.cmp(&right.bundle_id))
    });
    normalized
}

fn app_identity_key(app: &AppRecord) -> String {
    app.bundle_id
        .as_deref()
        .map(|bundle| format!("bundle:{}", bundle.to_ascii_lowercase()))
        .unwrap_or_else(|| format!("name:{}", app.name.to_ascii_lowercase()))
}

fn merge_app_record(existing: &mut AppRecord, incoming: &AppRecord) {
    if existing.bundle_id.is_none() {
        existing.bundle_id = incoming.bundle_id.clone();
    }

    if existing.pid.is_none() || incoming.pid < existing.pid {
        existing.pid = incoming.pid.or(existing.pid);
    }

    existing.is_running |= incoming.is_running;

    if app_display_name_score(incoming) > app_display_name_score(existing) {
        existing.name = incoming.name.clone();
    }
}

fn app_display_name_score(app: &AppRecord) -> u8 {
    let name = app.name.trim();
    if name.is_empty() || name.starts_with("pid-") {
        return 0;
    }

    match app.bundle_id.as_deref() {
        Some(bundle_id) if name.eq_ignore_ascii_case(bundle_id) => 1,
        _ => 2,
    }
}

fn is_listable_app_info(app: &AppInfo) -> bool {
    app.bundle_id.is_some() || !app.name.starts_with("pid-")
}

fn resolved_running_app_name(app: &AppInfo, labels: &BTreeMap<String, String>) -> String {
    app.bundle_id
        .as_ref()
        .and_then(|bundle| labels.get(bundle))
        .cloned()
        .unwrap_or_else(|| app.name.clone())
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

fn candidate_matches_app_selector(
    candidate: &NormalizedWindowCandidate,
    filter: &str,
    labels: &BTreeMap<String, String>,
) -> bool {
    candidate
        .app
        .as_ref()
        .is_some_and(|app| app_matches(app, filter))
        || candidate
            .window
            .app_name
            .as_deref()
            .is_some_and(|name| normalize_match_text(name) == filter)
        || candidate
            .app
            .as_ref()
            .and_then(|app| app.bundle_id.as_deref())
            .and_then(|bundle_id| labels.get(bundle_id))
            .is_some_and(|label| normalize_match_text(label) == filter)
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
