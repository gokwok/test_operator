use std::process::Command;

use operator_core::{OperatorError, PermissionStatus, PermissionsReport};

pub trait PermissionReader: Send + Sync {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPermissionReader;

impl PermissionReader for SystemPermissionReader {
    fn current_permissions(&self) -> Result<PermissionsReport, OperatorError> {
        Ok(PermissionsReport {
            accessibility: accessibility_status(),
            system_events: system_events_status(),
            screen_recording: screen_recording_status(),
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
