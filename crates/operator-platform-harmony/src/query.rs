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
        QueryRequest::ListApps { mode, filter } => {
            let apps = match mode {
                AppListMode::Running => normalize_running_apps(worker.query_windows().await?),
                AppListMode::All => {
                    let report = worker.query_apps().await?;
                    let windows = worker.query_windows().await?;
                    normalize_all_apps(report.bundles, windows)
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
    apps.into_iter().filter(|app| filter.matches(app)).collect()
}
