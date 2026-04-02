use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use operator_core::{
    Action, ActionOutcome, ActionRequest, ActionVerification, Capability, DigestOptions,
    ElementDigest, ExecContext, FocusInfo, ImageSizePx, Locator, ObserveRequest, ObserveResult,
    OperatorError, PlatformDriver, Point, QueryRequest, QueryResult, TargetDescriptor, TargetId,
    WindowInfo,
};
use tokio::time;

use crate::PlatformRegistry;
use crate::{
    ArtifactStore, AuditEvent, AuditEventKind, EventSink, RuntimeConfig, SessionStore,
    SnapshotStore, TargetResolver, ToolRegistry,
};

pub struct RuntimeCore {
    pub(crate) resolver: TargetResolver,
    pub(crate) platform_registry: PlatformRegistry,
    pub(crate) artifacts: Arc<dyn ArtifactStore>,
    pub(crate) snapshots: Arc<dyn SnapshotStore>,
    pub(crate) sessions: Arc<dyn SessionStore>,
    pub(crate) event_sink: Arc<dyn EventSink>,
    pub(crate) config: RuntimeConfig,
    pub(crate) driver_cache: Mutex<HashMap<TargetId, Arc<dyn PlatformDriver>>>,
}

impl RuntimeCore {
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn snapshots(&self) -> Arc<dyn SnapshotStore> {
        Arc::clone(&self.snapshots)
    }

    pub fn artifacts(&self) -> Arc<dyn ArtifactStore> {
        Arc::clone(&self.artifacts)
    }

    pub fn sessions(&self) -> Arc<dyn SessionStore> {
        Arc::clone(&self.sessions)
    }

    pub fn event_sink(&self) -> Arc<dyn EventSink> {
        Arc::clone(&self.event_sink)
    }

    pub fn resolve_target(
        &self,
        target: Option<&TargetId>,
    ) -> Result<TargetDescriptor, OperatorError> {
        self.resolver.resolve(target)
    }

    pub fn resolve_driver(
        &self,
        target: &TargetId,
    ) -> Result<(TargetDescriptor, Arc<dyn PlatformDriver>), OperatorError> {
        let descriptor = self.resolve_target(Some(target))?;
        if let Some(driver) = self
            .driver_cache
            .lock()
            .expect("runtime driver cache poisoned")
            .get(target)
            .cloned()
        {
            return Ok((descriptor, driver));
        }

        let factory = self
            .platform_registry
            .factory(&descriptor.driver)
            .ok_or_else(|| OperatorError::DriverUnavailable {
                target: descriptor.id.to_string(),
                driver: descriptor.driver.clone(),
            })?;
        let driver = factory.build(&descriptor)?;

        self.driver_cache
            .lock()
            .expect("runtime driver cache poisoned")
            .insert(target.clone(), driver.clone());

        Ok((descriptor, driver))
    }

    pub async fn observe(
        &self,
        req: ObserveRequest,
        ctx: ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        let (_, driver) = self.resolve_driver(&ctx.target)?;

        self.ensure_observe_capabilities(driver.as_ref(), &req, &ctx)
            .await?;
        self.emit_invoked("observe", &req, &ctx).await?;

        let started = Instant::now();
        let timeout_ms = self.timeout_ms(&ctx);
        let result =
            time::timeout(Duration::from_millis(timeout_ms), driver.observe(req, &ctx)).await;

        let observed = match result {
            Ok(Ok(observed)) => observed,
            Ok(Err(error)) => {
                self.emit_completed("observe", started, false, &ctx).await?;
                return Err(error);
            }
            Err(_) => {
                self.emit_completed("observe", started, false, &ctx).await?;
                return Err(OperatorError::Timeout { timeout_ms });
            }
        };

        let save_result = self.snapshots.save(&observed.snapshot).await;
        if save_result.is_err() {
            self.emit_completed("observe", started, false, &ctx).await?;
            return save_result.map(|_| observed);
        }

        self.emit_completed("observe", started, true, &ctx).await?;
        Ok(observed)
    }

    pub async fn query(
        &self,
        req: QueryRequest,
        ctx: ExecContext,
    ) -> Result<QueryResult, OperatorError> {
        let (_, driver) = self.resolve_driver(&ctx.target)?;

        self.ensure_query_capability(driver.as_ref(), &req, &ctx)
            .await?;
        self.emit_invoked("query", &req, &ctx).await?;

        if matches!(req, QueryRequest::Capabilities) {
            let result = QueryResult::Capabilities(driver.capabilities());
            self.emit_completed("query", Instant::now(), true, &ctx)
                .await?;
            return Ok(result);
        }

        let started = Instant::now();
        let timeout_ms = self.timeout_ms(&ctx);
        let result =
            time::timeout(Duration::from_millis(timeout_ms), driver.query(req, &ctx)).await;

        match result {
            Ok(Ok(result)) => {
                self.emit_completed("query", started, true, &ctx).await?;
                Ok(result)
            }
            Ok(Err(error)) => {
                self.emit_completed("query", started, false, &ctx).await?;
                Err(error)
            }
            Err(_) => {
                self.emit_completed("query", started, false, &ctx).await?;
                Err(OperatorError::Timeout { timeout_ms })
            }
        }
    }

    pub async fn act(
        &self,
        req: ActionRequest,
        ctx: ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        let (_, driver) = self.resolve_driver(&ctx.target)?;

        self.validate_action_request(&req)?;
        self.ensure_action_capability(driver.as_ref(), &req, &ctx)
            .await?;
        self.ensure_action_verification_capabilities(driver.as_ref(), &req, &ctx)
            .await?;
        self.emit_invoked("act", &req, &ctx).await?;

        let started = Instant::now();
        let timeout_ms = self.timeout_ms(&ctx);
        let normalized = match self.normalize_action_request(req).await {
            Ok(req) => req,
            Err(error) => {
                self.emit_completed("act", started, false, &ctx).await?;
                return Err(error);
            }
        };
        let result = time::timeout(
            Duration::from_millis(timeout_ms),
            driver.act(normalized.clone(), &ctx),
        )
        .await;

        match result {
            Ok(Ok(result)) => {
                let verified = match self
                    .verify_action_outcome(driver.as_ref(), &normalized, result, &ctx, timeout_ms)
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        self.emit_completed("act", started, false, &ctx).await?;
                        return Err(error);
                    }
                };
                self.emit_completed("act", started, true, &ctx).await?;
                Ok(verified)
            }
            Ok(Err(error)) => {
                self.emit_completed("act", started, false, &ctx).await?;
                Err(error)
            }
            Err(_) => {
                self.emit_completed("act", started, false, &ctx).await?;
                Err(OperatorError::Timeout { timeout_ms })
            }
        }
    }

    async fn ensure_observe_capabilities(
        &self,
        driver: &dyn PlatformDriver,
        req: &ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        if req.include_screenshot {
            self.require_capability(driver, Capability::Capture, "observe", ctx)
                .await?;
        }

        if req.include_elements {
            self.require_capability(driver, Capability::InspectTree, "observe", ctx)
                .await?;
        }

        Ok(())
    }

    async fn ensure_query_capability(
        &self,
        driver: &dyn PlatformDriver,
        req: &QueryRequest,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        let required = match req {
            QueryRequest::ListApps { .. } => Some(Capability::AppLifecycle),
            QueryRequest::ListWindows { .. } => Some(Capability::WindowQuery),
            QueryRequest::GetFocus => Some(Capability::InspectTree),
            QueryRequest::PermissionsStatus => Some(Capability::Permissions),
            QueryRequest::Capabilities => None,
        };

        if let Some(capability) = required {
            self.require_capability(driver, capability, "query", ctx)
                .await?;
        }

        Ok(())
    }

    async fn ensure_action_capability(
        &self,
        driver: &dyn PlatformDriver,
        req: &ActionRequest,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        let capability = match &req.action {
            Action::Click { .. }
            | Action::Move
            | Action::Scroll { .. }
            | Action::Drag { .. }
            | Action::Swipe { .. } => Capability::PointerInput,
            Action::Type { .. } | Action::Hotkey { .. } | Action::Press { .. } => {
                Capability::KeyboardInput
            }
            Action::LaunchApp { .. }
            | Action::SwitchApp
            | Action::QuitApp
            | Action::RelaunchApp
            | Action::HideApp
            | Action::UnhideApp => Capability::AppLifecycle,
            Action::CloseWindow
            | Action::MinimizeWindow
            | Action::MaximizeWindow
            | Action::MoveWindow { .. }
            | Action::ResizeWindow { .. }
            | Action::SetWindowBounds { .. }
            | Action::FocusWindow { .. } => Capability::WindowManagement,
        };

        self.require_capability(driver, capability, "act", ctx)
            .await
    }

    async fn ensure_action_verification_capabilities(
        &self,
        driver: &dyn PlatformDriver,
        req: &ActionRequest,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        let mut required = Vec::new();
        for verification in &req.verifications {
            match verification {
                ActionVerification::Focus => {
                    for capability in [Capability::WindowManagement, Capability::InspectTree] {
                        if !required.contains(&capability) {
                            required.push(capability);
                        }
                    }
                }
                ActionVerification::WindowState | ActionVerification::Geometry => {
                    if !required.contains(&Capability::WindowManagement) {
                        required.push(Capability::WindowManagement);
                    }
                }
            }
        }

        for capability in required {
            self.require_capability(driver, capability, "act", ctx)
                .await?;
        }

        Ok(())
    }

    async fn require_capability(
        &self,
        driver: &dyn PlatformDriver,
        capability: Capability,
        tool: &str,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        if driver.capabilities().supports(&capability) {
            return Ok(());
        }

        self.emit_audit(
            AuditEventKind::CapabilityDenied {
                tool: tool.to_string(),
                capability: capability.clone(),
            },
            ctx,
        )
        .await?;

        Err(OperatorError::CapabilityNotSupported(capability))
    }

    fn validate_action_request(&self, req: &ActionRequest) -> Result<(), OperatorError> {
        for verification in &req.verifications {
            let supported = match (&req.action, verification) {
                (Action::LaunchApp { .. } | Action::CloseWindow | Action::MaximizeWindow, _) => {
                    false
                }
                (Action::MinimizeWindow, ActionVerification::WindowState) => true,
                (Action::MinimizeWindow, _) => false,
                _ => true,
            };

            if !supported {
                return Err(OperatorError::Platform(format!(
                    "post-action {:?} verification is not supported for {}",
                    verification,
                    action_name(&req.action)
                )));
            }
        }

        if !req.verifications.is_empty()
            && req.target_selector.is_none()
            && !matches!(req.action, Action::FocusWindow { .. })
        {
            return Err(OperatorError::Platform(
                "post-action verification requires a target selector or focus-window action".into(),
            ));
        }

        match &req.action {
            Action::Move if req.locator.is_none() && req.target_selector.is_none() => {
                return Err(OperatorError::Platform(
                    "move requires a locator or target selector".into(),
                ));
            }
            Action::Drag {
                from:
                    Locator::SnapshotElement {
                        snapshot: from_snapshot,
                        ..
                    },
                to:
                    Locator::SnapshotElement {
                        snapshot: to_snapshot,
                        ..
                    },
                ..
            } => {
                if from_snapshot != to_snapshot {
                    return Err(OperatorError::Platform(
                        "drag: from/to must reference the same snapshot".into(),
                    ));
                }
            }
            Action::Swipe {
                from:
                    Locator::SnapshotElement {
                        snapshot: from_snapshot,
                        ..
                    },
                to:
                    Locator::SnapshotElement {
                        snapshot: to_snapshot,
                        ..
                    },
                ..
            } => {
                if from_snapshot != to_snapshot {
                    return Err(OperatorError::Platform(
                        "swipe: from/to must reference the same snapshot".into(),
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn verify_action_outcome(
        &self,
        driver: &dyn PlatformDriver,
        req: &ActionRequest,
        mut outcome: ActionOutcome,
        ctx: &ExecContext,
        timeout_ms: u64,
    ) -> Result<ActionOutcome, OperatorError> {
        if req.verifications.is_empty() {
            return Ok(outcome);
        }

        let mut cached_windows: Option<Vec<WindowInfo>> = None;
        let mut cached_focus: Option<Option<FocusInfo>> = None;

        for verification in &req.verifications {
            match verification {
                ActionVerification::Focus => {
                    if let Some(expected) = outcome.target_window.as_ref() {
                        let windows = self
                            .verification_windows(
                                driver,
                                ctx,
                                timeout_ms,
                                &mut cached_windows,
                                outcome
                                    .target_window
                                    .as_ref()
                                    .and_then(|window| window.app_name.clone())
                                    .or_else(|| {
                                        outcome.target_app.as_ref().map(|app| app.name.clone())
                                    }),
                            )
                            .await?;
                        let actual = find_window(&windows, expected.id).ok_or_else(|| {
                            OperatorError::Platform(format!(
                                "post-action focus verification failed: window {} was not found",
                                expected.id
                            ))
                        })?;
                        if !actual.is_focused {
                            return Err(OperatorError::Platform(format!(
                                "post-action focus verification failed: window {} is not focused",
                                expected.id
                            )));
                        }
                        outcome.target_window = Some(actual.clone());
                    } else {
                        let expected_app = outcome.target_app.as_ref().ok_or_else(|| {
                            OperatorError::Platform(
                                "post-action focus verification requires target app or window metadata"
                                    .into(),
                            )
                        })?;
                        let focused_app = self
                            .verification_focus_app(driver, ctx, timeout_ms, &mut cached_focus)
                            .await?;
                        if !focus_matches_expected_app(&focused_app, expected_app) {
                            let actual = focused_app
                                .as_ref()
                                .map(render_focus_app_identity)
                                .unwrap_or_else(|| "none".to_string());
                            return Err(OperatorError::Platform(format!(
                                "post-action focus verification failed: expected focused app {}, got {}",
                                render_expected_app_identity(expected_app),
                                actual,
                            )));
                        }
                    }
                }
                ActionVerification::WindowState => {
                    let expected = outcome.target_window.as_ref().ok_or_else(|| {
                        OperatorError::Platform(
                            "post-action window-state verification requires target window metadata"
                                .into(),
                        )
                    })?;
                    let windows = self
                        .verification_windows(
                            driver,
                            ctx,
                            timeout_ms,
                            &mut cached_windows,
                            expected.app_name.clone(),
                        )
                        .await?;
                    let actual = find_window(&windows, expected.id).ok_or_else(|| {
                        OperatorError::Platform(format!(
                            "post-action window-state verification failed: window {} was not found",
                            expected.id
                        ))
                    })?;
                    if actual.is_minimized != expected.is_minimized {
                        return Err(OperatorError::Platform(format!(
                            "post-action window-state verification failed: window {} minimized={} expected {}",
                            expected.id, actual.is_minimized, expected.is_minimized
                        )));
                    }
                    outcome.target_window = Some(actual.clone());
                }
                ActionVerification::Geometry => {
                    let expected = outcome.target_window.as_ref().ok_or_else(|| {
                        OperatorError::Platform(
                            "post-action geometry verification requires target window metadata"
                                .into(),
                        )
                    })?;
                    let expected_bounds = expected.bounds.ok_or_else(|| {
                        OperatorError::Platform(
                            "post-action geometry verification requires target window bounds in action outcome"
                                .into(),
                        )
                    })?;
                    let windows = self
                        .verification_windows(
                            driver,
                            ctx,
                            timeout_ms,
                            &mut cached_windows,
                            expected.app_name.clone(),
                        )
                        .await?;
                    let actual = find_window(&windows, expected.id).ok_or_else(|| {
                        OperatorError::Platform(format!(
                            "post-action geometry verification failed: window {} was not found",
                            expected.id
                        ))
                    })?;
                    if actual.bounds != Some(expected_bounds) {
                        return Err(OperatorError::Platform(format!(
                            "post-action geometry verification failed: window {} bounds did not match",
                            expected.id
                        )));
                    }
                    outcome.target_window = Some(actual.clone());
                }
            }
        }

        Ok(outcome)
    }

    async fn verification_windows(
        &self,
        driver: &dyn PlatformDriver,
        ctx: &ExecContext,
        timeout_ms: u64,
        cached_windows: &mut Option<Vec<WindowInfo>>,
        app: Option<String>,
    ) -> Result<Vec<WindowInfo>, OperatorError> {
        if cached_windows.is_none() {
            let result = self
                .query_with_timeout(driver, QueryRequest::ListWindows { app }, ctx, timeout_ms)
                .await?;
            let windows = match result {
                QueryResult::Windows(windows) => windows,
                _ => {
                    return Err(OperatorError::Platform(
                        "post-action verification expected windows query result".into(),
                    ))
                }
            };
            *cached_windows = Some(windows);
        }

        Ok(cached_windows.as_ref().unwrap().clone())
    }

    async fn verification_focus_app(
        &self,
        driver: &dyn PlatformDriver,
        ctx: &ExecContext,
        timeout_ms: u64,
        cached_focus: &mut Option<Option<FocusInfo>>,
    ) -> Result<Option<FocusInfo>, OperatorError> {
        if cached_focus.is_none() {
            let result = self
                .query_with_timeout(driver, QueryRequest::GetFocus, ctx, timeout_ms)
                .await?;
            let focused = match result {
                QueryResult::Focus(focus) => focus,
                _ => {
                    return Err(OperatorError::Platform(
                        "post-action verification expected focus query result".into(),
                    ))
                }
            };
            *cached_focus = Some(focused);
        }

        Ok(cached_focus.as_ref().unwrap().clone())
    }

    async fn query_with_timeout(
        &self,
        driver: &dyn PlatformDriver,
        req: QueryRequest,
        ctx: &ExecContext,
        timeout_ms: u64,
    ) -> Result<QueryResult, OperatorError> {
        match time::timeout(Duration::from_millis(timeout_ms), driver.query(req, ctx)).await {
            Ok(result) => result,
            Err(_) => Err(OperatorError::Timeout { timeout_ms }),
        }
    }

    async fn normalize_action_request(
        &self,
        mut req: ActionRequest,
    ) -> Result<ActionRequest, OperatorError> {
        if let Some(locator) = req.locator.take() {
            req.locator = Some(self.normalize_locator(locator).await?);
        }

        req.action = match req.action {
            Action::Drag { from, to, motion } => Action::Drag {
                from: self.normalize_locator(from).await?,
                to: self.normalize_locator(to).await?,
                motion,
            },
            Action::Swipe {
                from,
                to,
                duration_ms,
                steps,
            } => Action::Swipe {
                from: self.normalize_locator(from).await?,
                to: self.normalize_locator(to).await?,
                duration_ms,
                steps,
            },
            other => other,
        };

        Ok(req)
    }

    async fn normalize_locator(&self, locator: Locator) -> Result<Locator, OperatorError> {
        match locator {
            Locator::SnapshotElement { snapshot, element } => Ok(Locator::Coords(
                self.snapshot_element_point(&snapshot, &element).await?,
            )),
            Locator::SnapshotPixelCoords { snapshot, point } => Ok(Locator::Coords(
                self.snapshot_pixel_point(&snapshot, point).await?,
            )),
            Locator::SnapshotCoords { snapshot, point } => Ok(Locator::Coords(
                self.snapshot_relative_point(&snapshot, point).await?,
            )),
            Locator::SnapshotNormalizedCoords {
                snapshot,
                point,
                basis,
            } => Ok(Locator::Coords(
                self.snapshot_normalized_point(&snapshot, point, basis)
                    .await?,
            )),
            other => Ok(other),
        }
    }

    async fn snapshot_element_point(
        &self,
        snapshot: &operator_core::SnapshotId,
        element: &operator_core::ElementId,
    ) -> Result<Point, OperatorError> {
        let snapshot_record = self
            .snapshots
            .get(snapshot)
            .await?
            .ok_or_else(|| OperatorError::SnapshotNotFound(snapshot.clone()))?;

        // Both CLI users and the agent reference elements by display ID (e.g.
        // "e37") as shown in the rendered element digest.  Display IDs are
        // assigned transiently by ElementDigest and are not stored in the
        // snapshot's element map, so we rebuild the digest on demand to
        // resolve them to the underlying platform ID (ax-path).
        let resolved_id: operator_core::ElementId;
        let element = if snapshot_record.elements.contains_key(element) {
            element
        } else if let Some(native_id) = ElementDigest::from_snapshot(
            &snapshot_record,
            &DigestOptions::default(),
        )
        .as_ref()
        .and_then(|d| d.resolve_id(element.as_str()))
        {
            resolved_id = native_id.into();
            &resolved_id
        } else {
            return Err(OperatorError::ElementNotFound(element.clone()));
        };

        let element_record = snapshot_record
            .elements
            .get(element)
            .ok_or_else(|| OperatorError::ElementNotFound(element.clone()))?;
        let bounds = element_record.bounds.ok_or_else(|| {
            OperatorError::Platform(format!(
                "snapshot element {element} in {snapshot} has no bounds"
            ))
        })?;

        Ok(Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        })
    }

    async fn snapshot_relative_point(
        &self,
        snapshot: &operator_core::SnapshotId,
        point: Point,
    ) -> Result<Point, OperatorError> {
        let bounds = self.snapshot_capture_bounds(snapshot).await?;
        Ok(Point {
            x: bounds.x + point.x,
            y: bounds.y + point.y,
        })
    }

    async fn snapshot_pixel_point(
        &self,
        snapshot: &operator_core::SnapshotId,
        point: Point,
    ) -> Result<Point, OperatorError> {
        let bounds = self.snapshot_capture_bounds(snapshot).await?;

        if let Some(image_size) = self.snapshot_image_size(snapshot).await? {
            if image_size.width == 0 || image_size.height == 0 {
                return Err(OperatorError::Platform(format!(
                    "snapshot {snapshot} has invalid image_size_px for coordinate normalization"
                )));
            }

            return Ok(Point {
                x: bounds.x + bounds.width * (point.x / f64::from(image_size.width)),
                y: bounds.y + bounds.height * (point.y / f64::from(image_size.height)),
            });
        }

        let scale = self.snapshot_display_scale(snapshot).await?;
        if scale <= 0.0 {
            return Err(OperatorError::Platform(format!(
                "snapshot {snapshot} requires positive display_scale or image_size_px for pixel coordinate normalization"
            )));
        }

        Ok(Point {
            x: bounds.x + point.x / scale,
            y: bounds.y + point.y / scale,
        })
    }

    async fn snapshot_normalized_point(
        &self,
        snapshot: &operator_core::SnapshotId,
        point: Point,
        basis: f64,
    ) -> Result<Point, OperatorError> {
        if basis <= 0.0 {
            return Err(OperatorError::Platform(format!(
                "snapshot normalized coords for {snapshot} require a positive basis"
            )));
        }

        let bounds = self.snapshot_capture_bounds(snapshot).await?;
        Ok(Point {
            x: bounds.x + bounds.width * (point.x / basis),
            y: bounds.y + bounds.height * (point.y / basis),
        })
    }

    async fn snapshot_capture_bounds(
        &self,
        snapshot: &operator_core::SnapshotId,
    ) -> Result<operator_core::Rect, OperatorError> {
        let snapshot_record = self
            .snapshots
            .get(snapshot)
            .await?
            .ok_or_else(|| OperatorError::SnapshotNotFound(snapshot.clone()))?;
        snapshot_record.metadata.capture_bounds.ok_or_else(|| {
            OperatorError::Platform(format!(
                "snapshot {snapshot} has no capture bounds for coordinate normalization"
            ))
        })
    }

    async fn snapshot_image_size(
        &self,
        snapshot: &operator_core::SnapshotId,
    ) -> Result<Option<ImageSizePx>, OperatorError> {
        let snapshot_record = self
            .snapshots
            .get(snapshot)
            .await?
            .ok_or_else(|| OperatorError::SnapshotNotFound(snapshot.clone()))?;
        Ok(snapshot_record.metadata.image_size_px)
    }

    async fn snapshot_display_scale(
        &self,
        snapshot: &operator_core::SnapshotId,
    ) -> Result<f64, OperatorError> {
        let snapshot_record = self
            .snapshots
            .get(snapshot)
            .await?
            .ok_or_else(|| OperatorError::SnapshotNotFound(snapshot.clone()))?;
        snapshot_record
            .metadata
            .display_scale
            .map(f64::from)
            .ok_or_else(|| {
                OperatorError::Platform(format!(
                    "snapshot {snapshot} has no display_scale for coordinate normalization"
                ))
            })
    }

    async fn emit_invoked<T: serde::Serialize>(
        &self,
        tool: &str,
        input: &T,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        self.emit_audit(
            AuditEventKind::ToolInvoked {
                tool: tool.to_string(),
                input: serde_json::to_value(input)?,
            },
            ctx,
        )
        .await
    }

    async fn emit_completed(
        &self,
        tool: &str,
        started: Instant,
        success: bool,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        self.emit_audit(
            AuditEventKind::ToolCompleted {
                tool: tool.to_string(),
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                success,
            },
            ctx,
        )
        .await
    }

    async fn emit_audit(
        &self,
        kind: AuditEventKind,
        ctx: &ExecContext,
    ) -> Result<(), OperatorError> {
        if !self.config.audit_enabled {
            return Ok(());
        }

        self.event_sink
            .emit(AuditEvent {
                timestamp: std::time::SystemTime::now(),
                session_id: ctx.session.clone(),
                target_id: Some(ctx.target.clone()),
                kind,
            })
            .await
    }

    fn timeout_ms(&self, ctx: &ExecContext) -> u64 {
        ctx.timeout_ms.unwrap_or(self.config.default_timeout_ms)
    }
}

fn focus_matches_expected_app(
    focused_app: &Option<FocusInfo>,
    expected_app: &operator_core::AppInfo,
) -> bool {
    let Some(focused_app) = focused_app.as_ref() else {
        return false;
    };

    if let (Some(actual_bundle_id), Some(expected_bundle_id)) = (
        focused_app.bundle_id.as_deref(),
        expected_app.bundle_id.as_deref(),
    ) {
        if actual_bundle_id.eq_ignore_ascii_case(expected_bundle_id) {
            return true;
        }
    }

    focused_app
        .app_name
        .as_deref()
        .is_some_and(|actual_name| actual_name.eq_ignore_ascii_case(&expected_app.name))
}

fn render_expected_app_identity(app: &operator_core::AppInfo) -> String {
    match app.bundle_id.as_deref() {
        Some(bundle_id) => format!("{} ({bundle_id})", app.name),
        None => app.name.clone(),
    }
}

fn render_focus_app_identity(focus: &FocusInfo) -> String {
    match (focus.app_name.as_deref(), focus.bundle_id.as_deref()) {
        (Some(app_name), Some(bundle_id)) => format!("{app_name} ({bundle_id})"),
        (Some(app_name), None) => app_name.to_string(),
        (None, Some(bundle_id)) => bundle_id.to_string(),
        (None, None) => "unknown".to_string(),
    }
}

fn find_window(windows: &[WindowInfo], id: operator_core::WindowId) -> Option<&WindowInfo> {
    windows.iter().find(|window| window.id == id)
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Click { .. } => "click",
        Action::Move => "move",
        Action::Type { .. } => "type",
        Action::Press { .. } => "press",
        Action::Scroll { .. } => "scroll",
        Action::Hotkey { .. } => "hotkey",
        Action::Drag { .. } => "drag",
        Action::Swipe { .. } => "swipe",
        Action::LaunchApp { .. } => "launch-app",
        Action::CloseWindow => "close-window",
        Action::MinimizeWindow => "minimize-window",
        Action::MaximizeWindow => "maximize-window",
        Action::MoveWindow { .. } => "move-window",
        Action::ResizeWindow { .. } => "resize-window",
        Action::SetWindowBounds { .. } => "set-window-bounds",
        Action::SwitchApp => "switch-app",
        Action::QuitApp => "quit-app",
        Action::RelaunchApp => "relaunch-app",
        Action::HideApp => "hide-app",
        Action::UnhideApp => "unhide-app",
        Action::FocusWindow { .. } => "focus-window",
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub(crate) core: Arc<RuntimeCore>,
    pub(crate) tools: ToolRegistry,
}

impl Runtime {
    pub fn core(&self) -> Arc<RuntimeCore> {
        Arc::clone(&self.core)
    }

    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }
}
