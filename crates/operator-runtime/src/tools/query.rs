use std::sync::Arc;

use operator_core::{
    AppInfo, Capability, OperatorError, PermissionsReport, QueryRequest, QueryResult, WindowInfo,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    tools::{json_schema_for, ToolExecInput},
    RuntimeCore, ToolRegistration, ToolSpec,
};

const LIST_APPS_CAPABILITIES: &[Capability] = &[Capability::AppLifecycle];
const LIST_WINDOWS_CAPABILITIES: &[Capability] = &[Capability::WindowManagement];
const PERMISSIONS_STATUS_CAPABILITIES: &[Capability] = &[Capability::Permissions];

pub(crate) fn list_apps_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "list-apps",
            description: "List apps visible to the current target.",
            input_schema: json_schema_for::<ListAppsToolInput>(),
            output_schema: json_schema_for::<ListAppsToolOutput>(),
            capabilities_required: LIST_APPS_CAPABILITIES,
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { list_apps(input, core, ctx).await })
        }),
    }
}

pub(crate) fn list_windows_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "list-windows",
            description: "List windows, optionally filtered by app name.",
            input_schema: json_schema_for::<ListWindowsToolInput>(),
            output_schema: json_schema_for::<ListWindowsToolOutput>(),
            capabilities_required: LIST_WINDOWS_CAPABILITIES,
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { list_windows(input, core, ctx).await })
        }),
    }
}

pub(crate) fn permissions_status_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "permissions-status",
            description: "Read the current automation permission status for the target.",
            input_schema: json_schema_for::<PermissionsStatusToolInput>(),
            output_schema: json_schema_for::<PermissionsStatusToolOutput>(),
            capabilities_required: PERMISSIONS_STATUS_CAPABILITIES,
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { permissions_status(input, core, ctx).await })
        }),
    }
}

pub(crate) fn capabilities_registration() -> ToolRegistration {
    ToolRegistration {
        spec: ToolSpec {
            name: "capabilities",
            description: "List runtime capabilities exposed by the selected target.",
            input_schema: json_schema_for::<CapabilitiesToolInput>(),
            output_schema: json_schema_for::<CapabilitiesToolOutput>(),
            capabilities_required: &[],
            has_side_effects: false,
        },
        handler: Arc::new(|input, core, ctx| {
            Box::pin(async move { capabilities(input, core, ctx).await })
        }),
    }
}

async fn list_apps(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let _ = parse_input::<ListAppsToolInput>("list-apps", input)?;
    let result = core.query(QueryRequest::ListApps, ctx).await?;
    let QueryResult::Apps(apps) = result else {
        return unexpected_variant("list-apps", "apps");
    };

    serialize_output("list-apps", ListAppsToolOutput { apps })
}

async fn list_windows(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let input = parse_input::<ListWindowsToolInput>("list-windows", input)?;
    let result = core
        .query(QueryRequest::ListWindows { app: input.app }, ctx)
        .await?;
    let QueryResult::Windows(windows) = result else {
        return unexpected_variant("list-windows", "windows");
    };

    serialize_output("list-windows", ListWindowsToolOutput { windows })
}

async fn permissions_status(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let _ = parse_input::<PermissionsStatusToolInput>("permissions-status", input)?;
    let result = core.query(QueryRequest::PermissionsStatus, ctx).await?;
    let QueryResult::Permissions(permissions) = result else {
        return unexpected_variant("permissions-status", "permissions");
    };

    serialize_output(
        "permissions-status",
        PermissionsStatusToolOutput { permissions },
    )
}

async fn capabilities(
    input: Value,
    core: Arc<RuntimeCore>,
    ctx: operator_core::ExecContext,
) -> Result<Value, OperatorError> {
    let _ = parse_input::<CapabilitiesToolInput>("capabilities", input)?;
    let result = core.query(QueryRequest::Capabilities, ctx).await?;
    let QueryResult::Capabilities(capabilities) = result else {
        return unexpected_variant("capabilities", "capabilities");
    };

    let mut capabilities = capabilities.iter().cloned().collect::<Vec<_>>();
    capabilities.sort_by_key(capability_sort_key);

    serialize_output("capabilities", CapabilitiesToolOutput { capabilities })
}

fn parse_input<T: for<'de> Deserialize<'de>>(tool: &str, input: Value) -> Result<T, OperatorError> {
    serde_json::from_value(input).map_err(|error| OperatorError::Tool {
        tool: tool.to_string(),
        message: format!("invalid input: {error}"),
    })
}

fn serialize_output<T: Serialize>(tool: &str, output: T) -> Result<Value, OperatorError> {
    serde_json::to_value(output).map_err(|error| OperatorError::Tool {
        tool: tool.to_string(),
        message: format!("failed to serialize output: {error}"),
    })
}

fn unexpected_variant(tool: &str, expected: &str) -> Result<Value, OperatorError> {
    Err(OperatorError::Tool {
        tool: tool.to_string(),
        message: format!("runtime returned unexpected result variant, expected {expected}"),
    })
}

fn capability_sort_key(capability: &Capability) -> String {
    match capability {
        Capability::Capture => "Capture".into(),
        Capability::InspectTree => "InspectTree".into(),
        Capability::InspectText => "InspectText".into(),
        Capability::PointerInput => "PointerInput".into(),
        Capability::KeyboardInput => "KeyboardInput".into(),
        Capability::WindowManagement => "WindowManagement".into(),
        Capability::AppLifecycle => "AppLifecycle".into(),
        Capability::Clipboard => "Clipboard".into(),
        Capability::Permissions => "Permissions".into(),
        Capability::DeviceInfo => "DeviceInfo".into(),
        Capability::Extension(id) => format!("Extension:{}:{}", id.namespace, id.name),
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ListAppsToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ListAppsToolOutput {
    apps: Vec<AppInfo>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ListWindowsToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
    app: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct ListWindowsToolOutput {
    windows: Vec<WindowInfo>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct PermissionsStatusToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct PermissionsStatusToolOutput {
    permissions: PermissionsReport,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CapabilitiesToolInput {
    #[serde(flatten)]
    #[schemars(flatten)]
    exec: ToolExecInput,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
struct CapabilitiesToolOutput {
    capabilities: Vec<Capability>,
}
