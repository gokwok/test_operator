use std::process::Command;

use operator_core::{OperatorError, PermissionCheck, PermissionStatus, PermissionsReport};

pub(crate) const ACCESSIBILITY_CHECK_ID: &str = "accessibility";
pub(crate) const SYSTEM_EVENTS_CHECK_ID: &str = "system_events";
pub(crate) const SCREEN_RECORDING_CHECK_ID: &str = "screen_recording";

const ACCESSIBILITY_LABEL: &str = "Accessibility";
const SYSTEM_EVENTS_LABEL: &str = "System Events";
const SCREEN_RECORDING_LABEL: &str = "Screen Recording";

const ACCESSIBILITY_MESSAGE: &str = "Accessibility permission is required for macOS automation.";
const SYSTEM_EVENTS_MESSAGE: &str =
    "System Events access is required for macOS window queries and focus reads.";
const SCREEN_RECORDING_MESSAGE: &str = "Screen Recording permission is required for macOS capture.";

pub trait PermissionReader: Send + Sync {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPermissionReader;

impl PermissionReader for SystemPermissionReader {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError> {
        Ok(PermissionsReport::new([
            PermissionCheck::new(
                ACCESSIBILITY_CHECK_ID,
                ACCESSIBILITY_LABEL,
                accessibility_status(),
            )
            .with_message(ACCESSIBILITY_MESSAGE),
            PermissionCheck::new(
                SYSTEM_EVENTS_CHECK_ID,
                SYSTEM_EVENTS_LABEL,
                system_events_status(),
            )
            .with_message(SYSTEM_EVENTS_MESSAGE),
            PermissionCheck::new(
                SCREEN_RECORDING_CHECK_ID,
                SCREEN_RECORDING_LABEL,
                screen_recording_status(),
            )
            .with_message(SCREEN_RECORDING_MESSAGE),
        ]))
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
fn system_events_status() -> PermissionStatus {
    const SCRIPT: &str = r#"
const systemEvents = Application("System Events");
const processes = systemEvents.applicationProcesses.whose({frontmost: true})();
if (processes.length === 0) {
  0;
} else {
  const process = processes[0];
  const windows = process.windows();
  if (windows.length === 0) {
    0;
  } else {
    const window = windows[0];
    try { window.name(); } catch (error) {}
    try { window.position(); } catch (error) {}
    try { window.size(); } catch (error) {}
    windows.length;
  }
}
"#;

    match Command::new("osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .output()
    {
        Ok(output) if output.status.success() => PermissionStatus::Granted,
        Ok(_) => PermissionStatus::Denied,
        Err(_) => PermissionStatus::NotDetermined,
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
fn system_events_status() -> PermissionStatus {
    PermissionStatus::NotDetermined
}

#[cfg(not(target_os = "macos"))]
fn screen_recording_status() -> PermissionStatus {
    PermissionStatus::NotDetermined
}
