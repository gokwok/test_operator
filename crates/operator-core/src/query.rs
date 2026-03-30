use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CapabilitySet, Rect, WindowId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AppListMode {
    #[default]
    Running,
    All,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppListFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryRequest {
    ListApps {
        mode: AppListMode,
        #[serde(default)]
        filter: AppListFilter,
    },
    ListWindows {
        app: Option<String>,
    },
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

impl AppListFilter {
    pub fn matches(&self, app: &AppInfo) -> bool {
        if let Some(name) = self.name.as_ref() {
            let needle = name.to_lowercase();
            if !app.name.to_lowercase().contains(&needle) {
                return false;
            }
        }

        if let Some(bundle) = self.bundle.as_ref() {
            if app.bundle_id.as_deref() != Some(bundle.as_str()) {
                return false;
            }
        }

        true
    }
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
    pub checks: Vec<PermissionCheck>,
}

impl PermissionsReport {
    pub fn new<I>(checks: I) -> Self
    where
        I: IntoIterator<Item = PermissionCheck>,
    {
        Self {
            checks: checks.into_iter().collect(),
        }
    }

    pub fn check(&self, id: &str) -> Option<&PermissionCheck> {
        self.checks.iter().find(|check| check.id == id)
    }

    pub fn status(&self, id: &str) -> Option<PermissionStatus> {
        self.check(id).map(|check| check.status)
    }

    pub fn first_non_granted(&self) -> Option<&PermissionCheck> {
        self.checks
            .iter()
            .find(|check| check.status != PermissionStatus::Granted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PermissionCheck {
    pub id: String,
    pub label: String,
    pub status: PermissionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PermissionCheck {
    pub fn new(id: impl Into<String>, label: impl Into<String>, status: PermissionStatus) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status,
            message: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
}

#[cfg(test)]
mod tests {
    use super::{AppInfo, AppListFilter};

    #[test]
    fn app_list_filter_matches_name_by_contains() {
        let filter = AppListFilter {
            name: Some("code".into()),
            bundle: None,
        };
        let app = AppInfo {
            bundle_id: Some("com.openai.codex".into()),
            name: "Codex".into(),
            pid: Some(42),
            is_running: true,
        };

        assert!(filter.matches(&app));
    }

    #[test]
    fn app_list_filter_matches_bundle_by_exact_value() {
        let filter = AppListFilter {
            name: None,
            bundle: Some("com.apple.TextEdit".into()),
        };
        let app = AppInfo {
            bundle_id: Some("com.apple.TextEdit".into()),
            name: "TextEdit".into(),
            pid: Some(101),
            is_running: true,
        };

        assert!(filter.matches(&app));
        assert!(!AppListFilter {
            name: None,
            bundle: Some("com.apple.textedit".into()),
        }
        .matches(&app));
    }
}
