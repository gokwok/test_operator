use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, CapabilitySet, ExecContext, HealthStatus,
    Locator, MouseButton, ObserveRequest, ObserveResult, OperatorError, PermissionStatus,
    QueryRequest, QueryResult, Snapshot, SnapshotMetadata,
};

use crate::{
    locator::resolve_locator, AppService, CaptureProvider, InputSynthesizer, InspectResult,
    PermissionReader, SystemAppService, SystemCaptureProvider, SystemInputSynthesizer,
    SystemPermissionReader, SystemTreeInspector, TreeInspector,
};

static SNAPSHOT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct MacosDriver<
    A = SystemAppService,
    P = SystemPermissionReader,
    C = SystemCaptureProvider,
    I = SystemTreeInspector,
    S = SystemInputSynthesizer,
> {
    app_service: A,
    permission_reader: P,
    capture_provider: C,
    tree_inspector: I,
    input_synthesizer: S,
}

impl
    MacosDriver<
        SystemAppService,
        SystemPermissionReader,
        SystemCaptureProvider,
        SystemTreeInspector,
        SystemInputSynthesizer,
    >
{
    pub fn system() -> Self {
        Self::default()
    }
}

impl<A, P> MacosDriver<A, P, SystemCaptureProvider, SystemTreeInspector, SystemInputSynthesizer> {
    pub fn new(app_service: A, permission_reader: P) -> Self {
        Self::with_observe(
            app_service,
            permission_reader,
            SystemCaptureProvider,
            SystemTreeInspector,
        )
    }
}

impl<A, P, C, I> MacosDriver<A, P, C, I, SystemInputSynthesizer> {
    pub fn with_observe(
        app_service: A,
        permission_reader: P,
        capture_provider: C,
        tree_inspector: I,
    ) -> Self {
        Self::with_components(
            app_service,
            permission_reader,
            capture_provider,
            tree_inspector,
            SystemInputSynthesizer,
        )
    }
}

impl<A, P, C, I, S> MacosDriver<A, P, C, I, S> {
    pub fn with_components(
        app_service: A,
        permission_reader: P,
        capture_provider: C,
        tree_inspector: I,
        input_synthesizer: S,
    ) -> Self {
        Self {
            app_service,
            permission_reader,
            capture_provider,
            tree_inspector,
            input_synthesizer,
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

    pub fn input_synthesizer(&self) -> &S {
        &self.input_synthesizer
    }
}

impl Default
    for MacosDriver<
        SystemAppService,
        SystemPermissionReader,
        SystemCaptureProvider,
        SystemTreeInspector,
        SystemInputSynthesizer,
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
        Capability::PointerInput,
        Capability::KeyboardInput,
    ])
}

#[async_trait]
impl<A, P, C, I, S> operator_core::PlatformDriver for MacosDriver<A, P, C, I, S>
where
    A: AppService,
    P: PermissionReader,
    C: CaptureProvider,
    I: TreeInspector,
    S: InputSynthesizer,
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
            QueryRequest::GetFocus => Ok(QueryResult::Focus(self.app_service.get_focus()?)),
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
            Action::Click { button } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.click(req.locator, button, &permissions)
            }
            Action::Type { text } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.type_text(req.locator, &text, &permissions)
            }
            Action::Scroll { delta_x, delta_y } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.scroll(delta_x, delta_y, &permissions)
            }
            Action::Drag { from, to } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.drag(from, to, &permissions)
            }
            Action::Hotkey { keys } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.hotkey(&keys, &permissions)
            }
            Action::FocusWindow { id } => {
                self.app_service.focus_window(id)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some(format!("focused window {id}")),
                })
            }
        }
    }
}

impl<A, P, C, I, S> MacosDriver<A, P, C, I, S>
where
    P: PermissionReader,
    I: TreeInspector,
    S: InputSynthesizer,
{
    fn click(
        &self,
        locator: Option<Locator>,
        button: MouseButton,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let resolved = self.resolve_required_locator(locator, "click")?;
        self.input_synthesizer.click(resolved.point, button)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(action_detail("clicked", resolved.warning.as_deref())),
        })
    }

    fn type_text(
        &self,
        locator: Option<Locator>,
        text: &str,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let warning = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            self.input_synthesizer
                .click(resolved.point, MouseButton::Left)?;
            thread::sleep(std::time::Duration::from_millis(50));
            resolved.warning
        } else {
            None
        };
        self.input_synthesizer.type_text(text)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(action_detail("typed text", warning.as_deref())),
        })
    }

    fn scroll(
        &self,
        delta_x: f64,
        delta_y: f64,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        self.input_synthesizer.scroll(delta_x, delta_y)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some("scrolled".into()),
        })
    }

    fn drag(
        &self,
        from: Locator,
        to: Locator,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let from = resolve_locator(&from, &self.tree_inspector)?;
        let to = resolve_locator(&to, &self.tree_inspector)?;
        self.input_synthesizer.drag(from.point, to.point)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some("dragged".into()),
        })
    }

    fn hotkey(
        &self,
        keys: &[String],
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        self.input_synthesizer.hotkey(keys)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some("sent hotkey".into()),
        })
    }

    fn resolve_required_locator(
        &self,
        locator: Option<Locator>,
        action: &str,
    ) -> Result<crate::locator::ResolvedLocator, OperatorError> {
        let locator = locator
            .ok_or_else(|| OperatorError::Platform(format!("macOS {action} requires a locator")))?;
        resolve_locator(&locator, &self.tree_inspector)
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

fn require_accessibility_permission(
    permissions: &operator_core::PermissionsReport,
) -> Result<(), OperatorError> {
    if permissions.accessibility != PermissionStatus::Granted {
        return Err(OperatorError::PermissionDenied(
            "Accessibility permission is required for macOS input.".into(),
        ));
    }

    Ok(())
}

fn action_detail(action: &str, warning: Option<&str>) -> String {
    match warning {
        Some(warning) => format!("{action}; {warning}"),
        None => action.to_string(),
    }
}

fn next_snapshot_id() -> operator_core::SnapshotId {
    let counter = SNAPSHOT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();

    format!("snapshot-{timestamp}-{counter}").into()
}
