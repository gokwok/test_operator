use operator_core::{
    AppInfo, AppListFilter, AppListMode, Capability, CapabilitySet, OperatorError, QueryRequest,
    QueryResult,
};

use crate::{
    normalize::{normalize_all_apps, normalize_running_apps, normalize_windows},
    HarmonyHdcWorker,
};

pub(crate) async fn query(
    worker: &HarmonyHdcWorker,
    req: QueryRequest,
    capabilities: CapabilitySet,
) -> Result<QueryResult, OperatorError> {
    match req {
        QueryRequest::ListApps {
            mode,
            filter,
            flush,
        } => {
            if mode == AppListMode::All && filter.name.is_none() {
                if let Some(bundle) = filter.bundle.as_deref() {
                    return Ok(QueryResult::Apps(
                        query_exact_bundle_app(worker, bundle, flush).await?,
                    ));
                }
            }
            let apps = match mode {
                AppListMode::Running => {
                    let labels = worker.query_app_labels_map().await?;
                    normalize_running_apps(worker.query_windows().await?, &labels)
                }
                AppListMode::All => {
                    let report = worker.query_apps_with_refresh(flush).await?;
                    let windows = worker.query_windows().await?;
                    normalize_all_apps(report.installed_apps, windows, &report.labels)
                }
            };
            Ok(QueryResult::Apps(filter_app_infos(apps, &filter)))
        }
        QueryRequest::ListWindows { app } => Ok(QueryResult::Windows(normalize_windows(
            worker.query_windows().await?,
            app.as_deref(),
        ))),
        QueryRequest::PermissionsStatus => {
            Ok(QueryResult::Permissions(worker.permissions_report().await?))
        }
        QueryRequest::Capabilities => Ok(QueryResult::Capabilities(capabilities)),
        QueryRequest::GetFocus => Err(OperatorError::CapabilityNotSupported(
            Capability::InspectTree,
        )),
    }
}

fn filter_app_infos(apps: Vec<AppInfo>, filter: &AppListFilter) -> Vec<AppInfo> {
    apps.into_iter()
        .filter(|app| harmony_app_matches_filter(app, filter))
        .collect()
}

fn harmony_app_matches_filter(app: &AppInfo, filter: &AppListFilter) -> bool {
    if let Some(name) = filter.name.as_deref() {
        let needle = normalize_match_text(name);
        let name_matches = normalize_match_text(&app.name).contains(&needle);
        let bundle_matches = app
            .bundle_id
            .as_deref()
            .is_some_and(|bundle_id| normalize_match_text(bundle_id).contains(&needle));
        if !name_matches && !bundle_matches {
            return false;
        }
    }

    if let Some(bundle) = filter.bundle.as_deref() {
        if app.bundle_id.as_deref() != Some(bundle) {
            return false;
        }
    }

    true
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

async fn query_exact_bundle_app(
    worker: &HarmonyHdcWorker,
    bundle: &str,
    flush: bool,
) -> Result<Vec<AppInfo>, OperatorError> {
    let labels = worker.query_app_labels_map().await?;
    let windows = worker.query_windows().await?;
    let running = normalize_running_apps(windows, &labels);
    if let Some(app) = running
        .into_iter()
        .find(|app| app.bundle_id.as_deref() == Some(bundle))
    {
        return Ok(vec![app]);
    }

    let report = if flush {
        worker.query_apps_with_refresh(true).await?
    } else {
        match worker.cached_apps().await? {
            Some(report) => report,
            None => worker.query_apps_with_refresh(false).await?,
        }
    };

    let Some(app) = report
        .installed_apps
        .into_iter()
        .find(|app| app.bundle_id == bundle)
    else {
        return Ok(Vec::new());
    };
    Ok(vec![AppInfo {
        bundle_id: Some(app.bundle_id),
        name: app.name,
        pid: None,
        is_running: false,
    }])
}
