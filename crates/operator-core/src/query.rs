use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CapabilitySet, Rect, WindowId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryRequest {
    ListApps,
    ListWindows { app: Option<String> },
    GetFocus,
    PermissionsStatus,
    Capabilities,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum QueryResult {
    Apps(Vec<AppInfo>),
    Windows(Vec<WindowInfo>),
    Focus(Option<FocusInfo>),
    Permissions(PermissionsReport),
    Capabilities(CapabilitySet),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppInfo {
    pub bundle_id: Option<String>,
    pub name: String,
    pub pid: Option<u32>,
    pub is_running: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bounds: Option<Rect>,
    pub is_focused: bool,
    pub is_minimized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FocusInfo {
    pub role: String,
    pub label: Option<String>,
    pub bounds: Option<Rect>,
    pub bundle_id: Option<String>,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PermissionsReport {
    pub accessibility: PermissionStatus,
    pub system_events: PermissionStatus,
    pub screen_recording: PermissionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
}
