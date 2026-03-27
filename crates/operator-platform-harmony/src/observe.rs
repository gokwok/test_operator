use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use operator_core::{
    Capability, ExecContext, ObserveRequest, ObserveResult, OperatorError, Rect, Snapshot,
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
    if req.include_elements {
        return Err(OperatorError::CapabilityNotSupported(
            Capability::InspectTree,
        ));
    }

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

    let capture = worker.capture_observe(artifact_path).await?;
    let capture_bounds = capture_bounds(
        &req.surface.kind,
        capture.image_size_px.width,
        capture.image_size_px.height,
    )?;

    Ok(ObserveResult {
        snapshot: Snapshot {
            id: next_snapshot_id(),
            target: ctx.target.clone(),
            surface: req.surface,
            image_artifact: artifact_id,
            elements: HashMap::new(),
            root_ids: Vec::new(),
            metadata: SnapshotMetadata {
                platform: "harmony".into(),
                display_scale: None,
                capture_bounds: Some(capture_bounds),
                image_size_px: Some(capture.image_size_px),
                capture_duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            },
            created_at: SystemTime::now(),
            expires_at: None,
        },
    })
}

fn capture_bounds(surface: &SurfaceKind, width: u32, height: u32) -> Result<Rect, OperatorError> {
    match surface {
        SurfaceKind::Frontmost | SurfaceKind::Fullscreen { .. } => Ok(Rect {
            x: 0.0,
            y: 0.0,
            width: f64::from(width),
            height: f64::from(height),
        }),
        SurfaceKind::Window { .. } => Err(unsupported_surface_error("window")),
        SurfaceKind::Region { .. } => Err(unsupported_surface_error("region")),
    }
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
