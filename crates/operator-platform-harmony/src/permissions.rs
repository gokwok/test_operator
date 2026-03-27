use operator_core::{PermissionCheck, PermissionStatus, PermissionsReport};

pub const HDC_CONNECT_CHECK_ID: &str = "hdc.connect";
pub const HDC_SHELL_CHECK_ID: &str = "hdc.shell";
pub const HDC_CAPTURE_CHECK_ID: &str = "hdc.capture";
pub const HDC_UI_BRIDGE_CHECK_ID: &str = "hdc.ui_bridge";

const HDC_CONNECT_LABEL: &str = "HDC Connect";
const HDC_SHELL_LABEL: &str = "HDC Shell";
const HDC_CAPTURE_LABEL: &str = "HDC Capture";
const HDC_UI_BRIDGE_LABEL: &str = "HDC UI Bridge";

#[derive(Debug, Clone)]
pub(crate) struct ProbeStatus {
    pub(crate) status: PermissionStatus,
    pub(crate) message: Option<String>,
}

impl ProbeStatus {
    pub(crate) fn granted() -> Self {
        Self {
            status: PermissionStatus::Granted,
            message: None,
        }
    }

    pub(crate) fn denied(message: impl Into<String>) -> Self {
        Self {
            status: PermissionStatus::Denied,
            message: Some(message.into()),
        }
    }

    pub(crate) fn skipped(message: impl Into<String>) -> Self {
        Self {
            status: PermissionStatus::NotDetermined,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HarmonyPermissionSnapshot {
    pub(crate) connect: ProbeStatus,
    pub(crate) shell: ProbeStatus,
    pub(crate) capture: ProbeStatus,
    pub(crate) ui_bridge: ProbeStatus,
}

impl HarmonyPermissionSnapshot {
    pub(crate) fn report(&self) -> PermissionsReport {
        PermissionsReport::new([
            permission_check(HDC_CONNECT_CHECK_ID, HDC_CONNECT_LABEL, &self.connect),
            permission_check(HDC_SHELL_CHECK_ID, HDC_SHELL_LABEL, &self.shell),
            permission_check(HDC_CAPTURE_CHECK_ID, HDC_CAPTURE_LABEL, &self.capture),
            permission_check(HDC_UI_BRIDGE_CHECK_ID, HDC_UI_BRIDGE_LABEL, &self.ui_bridge),
        ])
    }
}

pub(crate) fn health_ready(permissions: &PermissionsReport) -> bool {
    permission_granted(permissions, HDC_CONNECT_CHECK_ID)
        && permission_granted(permissions, HDC_SHELL_CHECK_ID)
}

pub(crate) fn health_message(permissions: &PermissionsReport) -> Option<String> {
    if !health_ready(permissions) {
        return permissions
            .check(HDC_CONNECT_CHECK_ID)
            .filter(|check| check.status != PermissionStatus::Granted)
            .or_else(|| {
                permissions
                    .check(HDC_SHELL_CHECK_ID)
                    .filter(|check| check.status != PermissionStatus::Granted)
            })
            .and_then(|check| check.message.clone());
    }

    permissions
        .check(HDC_UI_BRIDGE_CHECK_ID)
        .filter(|check| check.status != PermissionStatus::Granted)
        .or_else(|| {
            permissions
                .check(HDC_CAPTURE_CHECK_ID)
                .filter(|check| check.status != PermissionStatus::Granted)
        })
        .and_then(|check| check.message.clone())
}

fn permission_check(id: &str, label: &str, probe: &ProbeStatus) -> PermissionCheck {
    let mut check = PermissionCheck::new(id, label, probe.status);
    if let Some(message) = &probe.message {
        check = check.with_message(message.clone());
    }
    check
}

fn permission_granted(permissions: &PermissionsReport, id: &str) -> bool {
    permissions.status(id) == Some(PermissionStatus::Granted)
}
