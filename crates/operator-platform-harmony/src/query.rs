use operator_core::{
    AppListMode, Capability, CapabilitySet, OperatorError, QueryRequest, QueryResult,
};

use crate::{
    normalize::{normalize_apps, normalize_windows},
    HarmonyHdcWorker,
};

pub(crate) async fn query(
    worker: &HarmonyHdcWorker,
    req: QueryRequest,
    capabilities: CapabilitySet,
) -> Result<QueryResult, OperatorError> {
    match req {
        QueryRequest::ListApps {
            mode: AppListMode::Running,
        } => {
            let report = worker.query_apps().await?;
            Ok(QueryResult::Apps(normalize_apps(
                report.bundles,
                report.current_app,
            )))
        }
        QueryRequest::ListApps {
            mode: AppListMode::All,
        } => Err(OperatorError::Platform(
            "app list --all is not supported for Harmony targets".into(),
        )),
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
