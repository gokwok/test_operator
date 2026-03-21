use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, CapabilitySet, ExecContext, HealthStatus,
    ObserveRequest, ObserveResult, OperatorError, PermissionStatus, QueryRequest, QueryResult,
    Snapshot, SnapshotMetadata,
};

use crate::{
    AppService, CaptureProvider, InspectResult, PermissionReader, SystemAppService,
    SystemCaptureProvider, SystemPermissionReader, SystemTreeInspector, TreeInspector,
};

static SNAPSHOT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct MacosDriver<
    A = SystemAppService,
    P = SystemPermissionReader,
    C = SystemCaptureProvider,
    I = SystemTreeInspector,
> {
    app_service: A,
    permission_reader: P,
    capture_provider: C,
    tree_inspector: I,
}

impl
    MacosDriver<
        SystemAppService,
        SystemPermissionReader,
        SystemCaptureProvider,
        SystemTreeInspector,
    >
{
    pub fn system() -> Self {
        Self::default()
    }
}

impl<A, P> MacosDriver<A, P, SystemCaptureProvider, SystemTreeInspector> {
    pub fn new(app_service: A, permission_reader: P) -> Self {
        Self::with_observe(
            app_service,
            permission_reader,
            SystemCaptureProvider,
            SystemTreeInspector,
        )
    }
}

impl<A, P, C, I> MacosDriver<A, P, C, I> {
    pub fn with_observe(
        app_service: A,
        permission_reader: P,
        capture_provider: C,
        tree_inspector: I,
    ) -> Self {
        Self {
            app_service,
            permission_reader,
            capture_provider,
            tree_inspector,
        }
    }

    pub fn app_service(&self) -> &A {
        &self.app_service
    }

    pub fn permission_reader(&self) -> &P {
        &self.permission_reader
    }

    pub fn capture_provider(&self) -> &C {
        &self.capture_provider
    }

    pub fn tree_inspector(&self) -> &I {
        &self.tree_inspector
    }
}

impl Default
    for MacosDriver<
        SystemAppService,
        SystemPermissionReader,
        SystemCaptureProvider,
        SystemTreeInspector,
    >
{
    fn default() -> Self {
        Self::new(SystemAppService, SystemPermissionReader)
    }
}

fn macos_capabilities() -> CapabilitySet {
    CapabilitySet::new([
        Capability::AppLifecycle,
        Capability::Capture,
        Capability::InspectTree,
        Capability::WindowManagement,
        Capability::Permissions,
    ])
}

#[async_trait]
impl<A, P, C, I> operator_core::PlatformDriver for MacosDriver<A, P, C, I>
where
    A: AppService,
    P: PermissionReader,
    C: CaptureProvider,
    I: TreeInspector,
{
    fn platform_id(&self) -> &'static str {
        "macos"
    }

    fn capabilities(&self) -> CapabilitySet {
        macos_capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        let permissions = self.permission_reader.current_permissions()?;
        let accessibility_granted = permissions.accessibility == PermissionStatus::Granted;
        let screen_recording_granted = permissions.screen_recording == PermissionStatus::Granted;
        let healthy = accessibility_granted && screen_recording_granted;
        let message = if !accessibility_granted {
            Some("Accessibility permission is required for macOS automation.".into())
        } else if !screen_recording_granted {
            Some("Screen Recording permission is required for macOS capture.".into())
        } else {
            None
        };

        Ok(HealthStatus {
            healthy,
            message,
            permissions,
        })
    }

    async fn observe(
        &self,
        req: ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        let permissions = self.permission_reader.current_permissions()?;
        require_observe_permissions(&permissions, &req)?;
        let started = Instant::now();

        let capture = if req.include_screenshot {
            Some(self.capture_provider.capture(&req.surface)?)
        } else {
            None
        };

        let inspection = if req.include_elements {
            self.tree_inspector.inspect(&req.surface)?
        } else {
            InspectResult {
                elements: Default::default(),
                root_ids: Vec::new(),
            }
        };

        Ok(ObserveResult {
            snapshot: Snapshot {
                id: next_snapshot_id(),
                target: ctx.target.clone(),
                surface: req.surface,
                image_artifact: capture.as_ref().map(|result| result.artifact_id.clone()),
                elements: inspection.elements,
                root_ids: inspection.root_ids,
                metadata: SnapshotMetadata {
                    platform: "macos".into(),
                    display_scale: capture.and_then(|result| result.display_scale),
                    capture_duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX))
                        as u64,
                },
                created_at: SystemTime::now(),
                expires_at: None,
            },
        })
    }

    async fn query(
        &self,
        req: QueryRequest,
        _ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError> {
        match req {
            QueryRequest::ListApps => Ok(QueryResult::Apps(self.app_service.list_apps()?)),
            QueryRequest::ListWindows { app } => Ok(QueryResult::Windows(
                self.app_service.list_windows(app.as_deref())?,
            )),
            QueryRequest::PermissionsStatus => Ok(QueryResult::Permissions(
                self.permission_reader.current_permissions()?,
            )),
            QueryRequest::Capabilities => Ok(QueryResult::Capabilities(self.capabilities())),
            QueryRequest::GetFocus => Err(OperatorError::CapabilityNotSupported(
                Capability::InspectTree,
            )),
        }
    }

    async fn act(
        &self,
        req: ActionRequest,
        _ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        match req.action {
            Action::LaunchApp { bundle_id_or_name } => {
                self.app_service.launch_app(&bundle_id_or_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some(format!("launched {bundle_id_or_name}")),
                })
            }
            Action::Click { .. } | Action::Scroll { .. } | Action::Drag { .. } => Err(
                OperatorError::CapabilityNotSupported(Capability::PointerInput),
            ),
            Action::Type { .. } | Action::Hotkey { .. } => Err(
                OperatorError::CapabilityNotSupported(Capability::KeyboardInput),
            ),
            Action::FocusWindow { .. } => Err(OperatorError::Platform(
                "focus-window is not implemented for the macOS foundation driver".into(),
            )),
        }
    }
}

fn require_observe_permissions(
    permissions: &operator_core::PermissionsReport,
    req: &ObserveRequest,
) -> Result<(), OperatorError> {
    if req.include_screenshot && permissions.screen_recording != PermissionStatus::Granted {
        return Err(OperatorError::PermissionDenied(
            "Screen Recording permission is required for macOS capture.".into(),
        ));
    }

    if req.include_elements && permissions.accessibility != PermissionStatus::Granted {
        return Err(OperatorError::PermissionDenied(
            "Accessibility permission is required for macOS tree inspection.".into(),
        ));
    }

    Ok(())
}

fn next_snapshot_id() -> operator_core::SnapshotId {
    let counter = SNAPSHOT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();

    format!("snapshot-{timestamp}-{counter}").into()
}
