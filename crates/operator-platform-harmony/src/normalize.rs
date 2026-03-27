use std::collections::BTreeSet;

use hmdriver_rs::{CorrelatedWindow, CorrelatedWindowList, CurrentApp, WindowRect};
use operator_core::{AppInfo, Rect, WindowInfo};

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
    let focused_window_id = windows.focused_window_id;
    let app_filter = app_filter.map(normalize_match_text);
    let mut normalized = windows
        .windows
        .into_iter()
        .filter_map(|entry| normalize_window(entry, focused_window_id, app_filter.as_deref()))
        .collect::<Vec<_>>();
    normalized.sort_by_key(|window| window.id.0);
    normalized
}

fn normalize_window(
    entry: CorrelatedWindow,
    focused_window_id: Option<u32>,
    app_filter: Option<&str>,
) -> Option<WindowInfo> {
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

    Some(WindowInfo {
        id: u64::from(entry.window.window_id).into(),
        title: non_empty(entry.window.name),
        app_name,
        bounds: rect_from_window(entry.window.rect),
        is_focused: focused_window_id == Some(entry.window.window_id),
        is_minimized: false,
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
