use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use operator_core::{ArtifactId, OperatorError, Rect, Surface, SurfaceKind};

static CAPTURE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub trait CaptureProvider: Send + Sync {
    fn capture(&self, surface: &Surface) -> Result<CaptureResult, OperatorError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureResult {
    pub artifact_id: ArtifactId,
    pub display_scale: Option<f32>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemCaptureProvider;

impl CaptureProvider for SystemCaptureProvider {
    fn capture(&self, surface: &Surface) -> Result<CaptureResult, OperatorError> {
        // SnapshotStore can resolve artifact ids but does not yet expose an artifact writer.
        // Keep raw captures in a temp cache and return the logical filename for now.
        let artifact_id = next_artifact_id();
        let cache_dir = capture_cache_dir();
        fs::create_dir_all(&cache_dir)?;

        let path = cache_dir.join(&artifact_id.0);
        capture_to_path(surface, &path)?;

        Ok(CaptureResult {
            artifact_id,
            display_scale: None,
        })
    }
}

fn capture_cache_dir() -> PathBuf {
    std::env::temp_dir().join("operator-macos-captures")
}

fn next_artifact_id() -> ArtifactId {
    let counter = CAPTURE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();

    ArtifactId(format!("capture-{timestamp}-{counter}.png"))
}

#[cfg(target_os = "macos")]
fn capture_to_path(surface: &Surface, path: &Path) -> Result<(), OperatorError> {
    let mut command = Command::new("screencapture");
    command.arg("-x");

    match &surface.kind {
        SurfaceKind::Fullscreen { display_id } => {
            if let Some(display_id) = display_id {
                command.arg("-D").arg(display_id.to_string());
            }
        }
        SurfaceKind::Frontmost => {
            if let Some(rect) = frontmost_window_bounds()? {
                command.arg("-R").arg(rect_argument(&rect));
            }
        }
        SurfaceKind::Window { id } => {
            command.arg("-l").arg(id.0.to_string());
        }
        SurfaceKind::Region { rect } => {
            command.arg("-R").arg(rect_argument(rect));
        }
    }

    let output = command.arg(path).output().map_err(|error| {
        OperatorError::Platform(format!("failed to invoke screencapture: {error}"))
    })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("Not authorized") || stderr.contains("not allowed") {
        return Err(OperatorError::PermissionDenied(stderr));
    }

    Err(OperatorError::Platform(format!(
        "screencapture failed: {stderr}"
    )))
}

#[cfg(target_os = "macos")]
fn frontmost_window_bounds() -> Result<Option<Rect>, OperatorError> {
    let script = r#"
tell application "System Events"
    tell first application process whose frontmost is true
        if (count of windows) is 0 then
            return ""
        end if
        tell front window
            set {xPos, yPos} to position
            set {winWidth, winHeight} to size
            return (xPos as string) & "," & (yPos as string) & "," & (winWidth as string) & "," & (winHeight as string)
        end tell
    end tell
end tell
"#;

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|error| OperatorError::Platform(format!("failed to invoke osascript: {error}")))?;

    let stdout = command_output("osascript", output)?;
    if stdout.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = stdout.split(',').collect();
    if parts.len() != 4 {
        return Err(OperatorError::Platform(format!(
            "unexpected frontmost window bounds: {stdout}"
        )));
    }

    Ok(Some(Rect {
        x: parse_number(parts[0], "x")?,
        y: parse_number(parts[1], "y")?,
        width: parse_number(parts[2], "width")?,
        height: parse_number(parts[3], "height")?,
    }))
}

#[cfg(target_os = "macos")]
fn parse_number(value: &str, field: &str) -> Result<f64, OperatorError> {
    value.trim().parse::<f64>().map_err(|error| {
        OperatorError::Platform(format!(
            "failed to parse {field} from osascript output: {error}"
        ))
    })
}

#[cfg(target_os = "macos")]
fn rect_argument(rect: &Rect) -> String {
    format!(
        "{},{},{},{}",
        rect.x.round() as i64,
        rect.y.round() as i64,
        rect.width.round() as i64,
        rect.height.round() as i64,
    )
}

#[cfg(target_os = "macos")]
fn command_output(command: &str, output: std::process::Output) -> Result<String, OperatorError> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("Not authorized") || stderr.contains("not allowed") {
        return Err(OperatorError::PermissionDenied(stderr));
    }

    Err(OperatorError::Platform(format!(
        "{command} failed: {stderr}"
    )))
}

#[cfg(not(target_os = "macos"))]
fn capture_to_path(_surface: &Surface, _path: &Path) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS capture is unavailable on non-macOS hosts".into(),
    ))
}
