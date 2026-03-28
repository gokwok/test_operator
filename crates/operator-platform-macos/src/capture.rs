use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use operator_core::{ArtifactId, ImageSizePx, OperatorError, Rect, Surface, SurfaceKind};

use crate::apps::{is_synthetic_window_id, resolve_window_record};

static CAPTURE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub trait CaptureProvider: Send + Sync {
    fn capture(&self, surface: &Surface) -> Result<CaptureResult, OperatorError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureResult {
    pub artifact_id: ArtifactId,
    pub display_scale: Option<f32>,
    pub capture_bounds: Option<Rect>,
    pub image_size_px: Option<ImageSizePx>,
}

#[derive(Debug, Clone)]
pub struct SystemCaptureProvider {
    artifacts_dir: PathBuf,
}

impl SystemCaptureProvider {
    pub fn new(artifacts_dir: impl AsRef<Path>) -> Self {
        Self {
            artifacts_dir: artifacts_dir.as_ref().to_path_buf(),
        }
    }

    fn artifact_path(&self, id: &ArtifactId) -> PathBuf {
        self.artifacts_dir.join(&id.0)
    }
}

impl Default for SystemCaptureProvider {
    fn default() -> Self {
        Self::new(default_artifacts_dir())
    }
}

impl CaptureProvider for SystemCaptureProvider {
    fn capture(&self, surface: &Surface) -> Result<CaptureResult, OperatorError> {
        let artifact_id = next_artifact_id();
        fs::create_dir_all(&self.artifacts_dir)?;

        let path = self.artifact_path(&artifact_id);
        let capture_bounds = capture_bounds_for_surface(surface)?;
        capture_to_path(surface, capture_bounds.as_ref(), &path)?;

        Ok(CaptureResult {
            artifact_id,
            display_scale: display_scale_from_path(&path),
            capture_bounds,
            image_size_px: image_size_from_path(&path),
        })
    }
}

fn default_artifacts_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("OPERATOR_HOME") {
        return PathBuf::from(path).join("artifacts");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".operator").join("artifacts");
    }

    PathBuf::from(".operator").join("artifacts")
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
fn capture_to_path(
    surface: &Surface,
    capture_bounds: Option<&Rect>,
    path: &Path,
) -> Result<(), OperatorError> {
    let mut command = Command::new("screencapture");
    command.args(capture_command_arguments(surface, capture_bounds)?);

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
fn capture_bounds_for_surface(surface: &Surface) -> Result<Option<Rect>, OperatorError> {
    match &surface.kind {
        SurfaceKind::Frontmost => frontmost_window_bounds(),
        SurfaceKind::Region { rect } => Ok(Some(*rect)),
        SurfaceKind::Window { id } if is_synthetic_window_id(*id) => {
            let window = resolve_window_record(*id)?;
            let bounds = window.bounds.ok_or_else(|| {
                OperatorError::Platform(format!("window {id} has no bounds available on macOS"))
            })?;
            Ok(Some(bounds))
        }
        SurfaceKind::Fullscreen { .. } | SurfaceKind::Window { .. } => Ok(None),
    }
}

#[cfg(target_os = "macos")]
fn capture_command_arguments(
    surface: &Surface,
    capture_bounds: Option<&Rect>,
) -> Result<Vec<String>, OperatorError> {
    let mut args = vec!["-x".to_string()];

    match &surface.kind {
        SurfaceKind::Fullscreen { display_id } => {
            if let Some(display_id) = display_id {
                args.push("-D".into());
                args.push(display_id.to_string());
            }
        }
        SurfaceKind::Frontmost => {
            if let Some(rect) = capture_bounds {
                args.push("-R".into());
                args.push(rect_argument(rect));
            }
        }
        SurfaceKind::Window { id } if is_synthetic_window_id(*id) => {
            let rect = capture_bounds.ok_or_else(|| {
                OperatorError::Platform(format!("window {id} has no bounds available on macOS"))
            })?;
            args.push("-R".into());
            args.push(rect_argument(rect));
        }
        SurfaceKind::Window { id } => {
            args.push("-l".into());
            args.push(id.0.to_string());
        }
        SurfaceKind::Region { rect } => {
            args.push("-R".into());
            args.push(rect_argument(rect));
        }
    }

    Ok(args)
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
fn capture_to_path(
    _surface: &Surface,
    _capture_bounds: Option<&Rect>,
    _path: &Path,
) -> Result<(), OperatorError> {
    Err(OperatorError::Platform(
        "macOS capture is unavailable on non-macOS hosts".into(),
    ))
}

#[cfg(not(target_os = "macos"))]
fn capture_bounds_for_surface(_surface: &Surface) -> Result<Option<Rect>, OperatorError> {
    Ok(None)
}

#[cfg(target_os = "macos")]
fn display_scale_from_path(path: &Path) -> Option<f32> {
    let output = Command::new("sips")
        .arg("-g")
        .arg("dpiWidth")
        .arg("-g")
        .arg("dpiHeight")
        .arg(path)
        .output()
        .ok()?;
    let stdout = command_output("sips", output).ok()?;

    display_scale_from_sips_output(&stdout)
}

#[cfg(not(target_os = "macos"))]
fn display_scale_from_path(_path: &Path) -> Option<f32> {
    None
}

#[cfg(target_os = "macos")]
fn image_size_from_path(path: &Path) -> Option<ImageSizePx> {
    let output = Command::new("sips")
        .arg("-g")
        .arg("pixelWidth")
        .arg("-g")
        .arg("pixelHeight")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    image_size_from_sips_output(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "macos"))]
fn image_size_from_path(_path: &Path) -> Option<ImageSizePx> {
    None
}

fn image_size_from_sips_output(output: &str) -> Option<ImageSizePx> {
    let width = output
        .lines()
        .find_map(|line| line.split_once("pixelWidth:"))
        .and_then(|(_, value)| value.trim().parse::<u32>().ok())?;
    let height = output
        .lines()
        .find_map(|line| line.split_once("pixelHeight:"))
        .and_then(|(_, value)| value.trim().parse::<u32>().ok())?;
    Some(ImageSizePx { width, height })
}

fn display_scale_from_sips_output(output: &str) -> Option<f32> {
    let width = parse_sips_value(output, "dpiWidth");
    let height = parse_sips_value(output, "dpiHeight");
    let dpi = match (width, height) {
        (Some(width), Some(height)) if width > 0.0 && height > 0.0 => (width + height) / 2.0,
        (Some(width), _) if width > 0.0 => width,
        (_, Some(height)) if height > 0.0 => height,
        _ => return None,
    };

    Some((dpi / 72.0) as f32)
}

fn parse_sips_value(output: &str, key: &str) -> Option<f64> {
    output.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        if label.trim() != key {
            return None;
        }

        value.trim().parse::<f64>().ok()
    })
}

#[cfg(test)]
mod tests {
    use operator_core::{Rect, Surface, SurfaceKind, WindowId};

    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn native_window_capture_uses_window_selector() {
        let args = capture_command_arguments(
            &Surface {
                kind: SurfaceKind::Window {
                    id: WindowId::from(42),
                },
            },
            None,
        )
        .unwrap();

        assert_eq!(args, vec!["-x", "-l", "42"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn synthetic_window_capture_uses_region_fallback() {
        let args = capture_command_arguments(
            &Surface {
                kind: SurfaceKind::Window {
                    id: WindowId((1 << 63) | 42),
                },
            },
            Some(&Rect {
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            }),
        )
        .unwrap();

        assert_eq!(args, vec!["-x", "-R", "10,20,300,200"]);
    }

    #[test]
    fn parses_retina_display_scale_from_sips_output() {
        let output = "  dpiWidth: 144.000\n  dpiHeight: 144.000\n";

        assert_eq!(display_scale_from_sips_output(output), Some(2.0));
    }

    #[test]
    fn parses_image_size_from_sips_output() {
        let output = "  pixelWidth: 460\n  pixelHeight: 816\n";

        assert_eq!(
            image_size_from_sips_output(output),
            Some(ImageSizePx {
                width: 460,
                height: 816,
            })
        );
    }

    #[test]
    fn system_capture_provider_uses_configured_artifact_directory() {
        let dir = std::env::temp_dir().join("operator-macos-capture-tests");
        let provider = SystemCaptureProvider::new(&dir);
        let artifact_id = ArtifactId("capture-1.png".into());

        assert_eq!(
            provider.artifact_path(&artifact_id),
            dir.join("capture-1.png")
        );
    }
}
