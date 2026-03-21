use operator_core::{OperatorError, PermissionStatus, PermissionsReport};

pub trait PermissionReader: Send + Sync {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPermissionReader;

impl PermissionReader for SystemPermissionReader {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError> {
        Ok(PermissionsReport {
            screen_recording: screen_recording_status(),
            accessibility: accessibility_status(),
        })
    }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGPreflightScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
fn accessibility_status() -> PermissionStatus {
    if unsafe { AXIsProcessTrusted() } {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

#[cfg(target_os = "macos")]
fn screen_recording_status() -> PermissionStatus {
    if unsafe { CGPreflightScreenCaptureAccess() } {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

#[cfg(not(target_os = "macos"))]
fn accessibility_status() -> PermissionStatus {
    PermissionStatus::NotDetermined
}

#[cfg(not(target_os = "macos"))]
fn screen_recording_status() -> PermissionStatus {
    PermissionStatus::NotDetermined
}
