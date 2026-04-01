use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use image::ImageFormat;
use operator_core::{
    ExecContext, ImageSizePx, ObserveRequest, ObserveResult, OperatorError, Rect, Snapshot,
    SnapshotMetadata, SurfaceKind,
};

use crate::HarmonyHdcWorker;

static SNAPSHOT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
static ARTIFACT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) async fn observe(
    worker: &HarmonyHdcWorker,
    artifacts_dir: &Path,
    req: ObserveRequest,
    ctx: &ExecContext,
) -> Result<ObserveResult, OperatorError> {
    let started = Instant::now();
    let artifact_id = if req.include_screenshot {
        Some(next_artifact_id())
    } else {
        None
    };
    let artifact_path = if let Some(artifact_id) = &artifact_id {
        std::fs::create_dir_all(artifacts_dir)?;
        Some(artifacts_dir.join(artifact_id.as_file_name()?))
    } else {
        None
    };

    let capture = worker
        .capture_observe(
            artifact_path.clone(),
            matches!(&req.surface.kind, SurfaceKind::Frontmost),
        )
        .await?;
    let display_bounds = fullscreen_bounds(capture.image_size_px);
    let frontmost_bounds =
        clamp_rect_to_image(capture.focused_window_bounds, capture.image_size_px);
    let capture_bounds = match &req.surface.kind {
        SurfaceKind::Frontmost => frontmost_bounds.unwrap_or(display_bounds),
        SurfaceKind::Fullscreen { .. } => display_bounds,
        SurfaceKind::Window { .. } => return Err(unsupported_surface_error("window")),
        SurfaceKind::Region { .. } => return Err(unsupported_surface_error("region")),
    };

    if let (SurfaceKind::Frontmost, Some(path), Some(bounds)) = (
        &req.surface.kind,
        artifact_path.as_deref(),
        frontmost_bounds,
    ) {
        crop_artifact_to_bounds(path, bounds)?;
    }

    let image_size_px = Some(match &req.surface.kind {
        SurfaceKind::Frontmost => image_size_from_bounds(capture_bounds),
        SurfaceKind::Fullscreen { .. } => capture.image_size_px,
        SurfaceKind::Window { .. } | SurfaceKind::Region { .. } => unreachable!(),
    });
    let inspection = if req.include_elements {
        let region = Some(capture_bounds);
        worker.inspect_tree(region).await?
    } else {
        crate::inspect::InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        }
    };
    let element_tree_assessment = req
        .include_elements
        .then(|| crate::inspect::assess_element_tree(&inspection))
        .flatten();

    Ok(ObserveResult {
        snapshot: Snapshot {
            id: next_snapshot_id(),
            target: ctx.target.clone(),
            surface: req.surface,
            image_artifact: artifact_id,
            elements: inspection.elements,
            root_ids: inspection.root_ids,
            metadata: SnapshotMetadata {
                platform: "harmony".into(),
                display_scale: None,
                capture_bounds: Some(capture_bounds),
                image_size_px,
                element_tree: element_tree_assessment,
                capture_duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            },
            created_at: SystemTime::now(),
            expires_at: None,
        },
    })
}

fn fullscreen_bounds(image_size_px: ImageSizePx) -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(image_size_px.width),
        height: f64::from(image_size_px.height),
    }
}

fn image_size_from_bounds(bounds: Rect) -> ImageSizePx {
    ImageSizePx {
        width: bounds.width.round() as u32,
        height: bounds.height.round() as u32,
    }
}

fn clamp_rect_to_image(bounds: Option<Rect>, image_size_px: ImageSizePx) -> Option<Rect> {
    let bounds = bounds?;
    let left = bounds.x.max(0.0).round();
    let top = bounds.y.max(0.0).round();
    let right = (bounds.x + bounds.width)
        .min(f64::from(image_size_px.width))
        .round();
    let bottom = (bounds.y + bounds.height)
        .min(f64::from(image_size_px.height))
        .round();

    if right <= left || bottom <= top {
        return None;
    }

    Some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

fn crop_artifact_to_bounds(path: &Path, bounds: Rect) -> Result<(), OperatorError> {
    let image = image::open(path).map_err(|error| {
        OperatorError::Platform(format!(
            "failed to decode Harmony screenshot artifact {}: {error}",
            path.display()
        ))
    })?;
    let cropped = image.crop_imm(
        bounds.x.round() as u32,
        bounds.y.round() as u32,
        bounds.width.round() as u32,
        bounds.height.round() as u32,
    );
    cropped
        .save_with_format(path, ImageFormat::Jpeg)
        .map_err(|error| {
            OperatorError::Platform(format!(
                "failed to crop Harmony screenshot artifact {}: {error}",
                path.display()
            ))
        })?;
    Ok(())
}

fn unsupported_surface_error(surface: &str) -> OperatorError {
    OperatorError::Platform(format!(
        "driver harmony.hdc only supports observe surfaces `frontmost` and `fullscreen` in the first phase, got `{surface}`"
    ))
}

fn next_snapshot_id() -> operator_core::SnapshotId {
    let counter = SNAPSHOT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();

    format!("snapshot-{timestamp}-{counter}").into()
}

fn next_artifact_id() -> operator_core::ArtifactId {
    let counter = ARTIFACT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();

    format!("capture-{timestamp}-{counter}.jpeg").into()
}
