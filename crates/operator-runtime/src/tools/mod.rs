mod action;
mod observe;
mod query;
mod snapshot_get;

use operator_core::{SessionId, TargetId};
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;

use crate::ToolRegistration;

pub(crate) fn registrations() -> Vec<ToolRegistration> {
    let mut registrations = vec![
        observe::registration(),
        snapshot_get::registration(),
        query::get_focus_registration(),
        query::list_apps_registration(),
        query::list_windows_registration(),
        query::permissions_status_registration(),
        query::capabilities_registration(),
    ];
    registrations.extend(action::registrations());
    registrations
}

pub(crate) fn json_schema_for<T: JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).expect("schemas should serialize")
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct ToolExecInput {
    pub target: Option<TargetId>,
    pub session_id: Option<SessionId>,
    pub timeout_ms: Option<u64>,
}
