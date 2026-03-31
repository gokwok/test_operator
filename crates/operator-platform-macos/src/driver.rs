use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionCoordinates, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionSideEffect,
    ActionTargetSelector, AppInfo, AppListFilter, AppListMode, Capability, CapabilitySet,
    ClickMode, DragMotion, ExecContext, HealthStatus, Locator, ObserveRequest, ObserveResult,
    OperatorError, PermissionStatus, Point, QueryRequest, QueryResult, Rect, Snapshot,
    SnapshotMetadata, Surface, SurfaceKind, TypeTrailingKey, WindowInfo,
};

use crate::{
    effects::ActionEffects, locator::resolve_locator, AppService, CaptureProvider,
    InputSynthesizer, InspectResult, PermissionReader, SystemAppService, SystemCaptureProvider,
    SystemInputSynthesizer, SystemPermissionReader, SystemTreeInspector, TreeInspector,
    WindowTarget, ACCESSIBILITY_CHECK_ID, SCREEN_RECORDING_CHECK_ID, SYSTEM_EVENTS_CHECK_ID,
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
    effects: ActionEffects,
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
            SystemCaptureProvider::default(),
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
            effects: ActionEffects::new(),
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
        Capability::WindowQuery,
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

    fn driver_id(&self) -> &str {
        "macos.system"
    }

    fn capabilities(&self) -> CapabilitySet {
        macos_capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        let permissions = self.permission_reader.current_permissions()?;
        let healthy = permissions.first_non_granted().is_none();
        let message = permissions
            .first_non_granted()
            .and_then(|check| check.message.clone());

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
        let resolved_surface = self.resolve_observe_surface(&req.surface);

        let capture = if req.include_screenshot {
            Some(match resolved_surface.window_target.as_ref() {
                Some(target) => self.capture_provider.capture_window_target(target)?,
                None => self
                    .capture_provider
                    .capture(&resolved_surface.capture_surface)?,
            })
        } else {
            None
        };

        let inspection = if req.include_elements {
            match resolved_surface.window_target.as_ref() {
                Some(target) => self.tree_inspector.inspect_window_target(target)?,
                None => self
                    .tree_inspector
                    .inspect(&resolved_surface.inspect_surface)?,
            }
        } else {
            InspectResult {
                elements: Default::default(),
                root_ids: Vec::new(),
            }
        };

        let image_artifact = capture.as_ref().map(|result| result.artifact_id.clone());
        let display_scale = capture.as_ref().and_then(|result| result.display_scale);
        let capture_bounds = capture
            .as_ref()
            .and_then(|result| result.capture_bounds)
            .or(resolved_surface.capture_bounds);
        let image_size_px = capture.as_ref().and_then(|result| result.image_size_px);

        Ok(ObserveResult {
            snapshot: Snapshot {
                id: next_snapshot_id(),
                target: ctx.target.clone(),
                surface: req.surface,
                image_artifact,
                elements: inspection.elements,
                root_ids: inspection.root_ids,
                metadata: SnapshotMetadata {
                    platform: "macos".into(),
                    display_scale,
                    capture_bounds,
                    image_size_px,
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
            QueryRequest::ListApps { mode, filter, .. } => Ok(QueryResult::Apps(filter_app_infos(
                self.app_service.list_apps(mode)?,
                &filter,
            ))),
            QueryRequest::ListWindows { app } => Ok(QueryResult::Windows({
                let permissions = self.permission_reader.current_permissions()?;
                require_system_events_permission(&permissions)?;
                self.app_service.list_windows(app.as_deref())?
            })),
            QueryRequest::PermissionsStatus => Ok(QueryResult::Permissions(
                self.permission_reader.current_permissions()?,
            )),
            QueryRequest::Capabilities => Ok(QueryResult::Capabilities(self.capabilities())),
            QueryRequest::GetFocus => {
                let permissions = self.permission_reader.current_permissions()?;
                require_system_events_permission(&permissions)?;
                Ok(QueryResult::Focus(self.app_service.get_focus()?))
            }
        }
    }

    async fn act(
        &self,
        req: ActionRequest,
        _ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError> {
        let ActionRequest {
            action,
            locator,
            target_selector,
            focus_policy,
            verifications: _,
        } = req;
        let target = ActionTargetConfig::new(target_selector.as_ref(), focus_policy);

        match action {
            Action::LaunchApp { bundle_id_or_name } => {
                self.app_service.launch_app(&bundle_id_or_name)?;
                let mut outcome =
                    successful_action_outcome(format!("launched {bundle_id_or_name}"));
                outcome.side_effects = vec![ActionSideEffect::LaunchApp];
                Ok(outcome)
            }
            Action::CloseWindow => {
                let window = self.window_action_target(target, "close-window")?;
                self.app_service.close_window(window.id)?;
                let mut outcome = successful_action_outcome(format!("closed window {}", window.id));
                outcome.target_window = Some(window);
                outcome.side_effects = vec![ActionSideEffect::CloseWindow];
                Ok(outcome)
            }
            Action::MinimizeWindow => {
                let window = self.window_action_target(target, "minimize-window")?;
                self.app_service.minimize_window(window.id)?;
                let mut outcome =
                    successful_action_outcome(format!("minimized window {}", window.id));
                outcome.target_window = Some(window_with_minimized(&window, true));
                outcome.side_effects = vec![ActionSideEffect::MinimizeWindow];
                Ok(outcome)
            }
            Action::MaximizeWindow => {
                let window = self.window_action_target(target, "maximize-window")?;
                self.app_service.maximize_window(window.id)?;
                let mut outcome =
                    successful_action_outcome(format!("maximized window {}", window.id));
                outcome.target_window = Some(window);
                outcome.side_effects = vec![ActionSideEffect::MaximizeWindow];
                Ok(outcome)
            }
            Action::MoveWindow { x, y } => {
                let window = self.window_action_target(target, "move-window")?;
                let bounds = self.app_service.move_window(window.id, x, y)?;
                let mut outcome =
                    successful_action_outcome(window_geometry_detail("moved", window.id, bounds));
                outcome.target_window = Some(window_with_bounds(&window, bounds));
                outcome.side_effects = vec![ActionSideEffect::MoveWindow { bounds }];
                Ok(outcome)
            }
            Action::ResizeWindow { width, height } => {
                let window = self.window_action_target(target, "resize-window")?;
                let bounds = self.app_service.resize_window(window.id, width, height)?;
                let mut outcome =
                    successful_action_outcome(window_geometry_detail("resized", window.id, bounds));
                outcome.target_window = Some(window_with_bounds(&window, bounds));
                outcome.side_effects = vec![ActionSideEffect::ResizeWindow { bounds }];
                Ok(outcome)
            }
            Action::SetWindowBounds { bounds } => {
                let window = self.window_action_target(target, "set-window-bounds")?;
                let bounds = self.app_service.set_window_bounds(window.id, bounds)?;
                let mut outcome =
                    successful_action_outcome(window_geometry_detail("set", window.id, bounds));
                outcome.target_window = Some(window_with_bounds(&window, bounds));
                outcome.side_effects = vec![ActionSideEffect::SetWindowBounds { bounds }];
                Ok(outcome)
            }
            Action::SwitchApp => {
                let prepared = self.lifecycle_action_target(target)?;
                let app_name = prepared.app_name()?;
                self.app_service.focus_app(&app_name)?;
                let mut outcome = successful_action_outcome("switched app");
                apply_prepared_target(&mut outcome, Some(&prepared));
                outcome.side_effects = vec![ActionSideEffect::SwitchApp];
                Ok(outcome)
            }
            Action::QuitApp => {
                let prepared = self.lifecycle_action_target(target)?;
                let app_name = prepared.app_name()?;
                self.app_service.quit_app(&app_name)?;
                let mut outcome = successful_action_outcome("quit app");
                apply_prepared_target(&mut outcome, Some(&prepared));
                outcome.side_effects = vec![ActionSideEffect::QuitApp];
                Ok(outcome)
            }
            Action::RelaunchApp => {
                let prepared = self.lifecycle_action_target(target)?;
                let app_name = prepared.app_name()?;
                self.app_service.relaunch_app(&app_name)?;
                let mut outcome = successful_action_outcome("relaunched app");
                apply_prepared_target(&mut outcome, Some(&prepared));
                outcome.side_effects = vec![ActionSideEffect::RelaunchApp];
                Ok(outcome)
            }
            Action::HideApp => {
                let prepared = self.lifecycle_action_target(target)?;
                let app_name = prepared.app_name()?;
                self.app_service.hide_app(&app_name)?;
                let mut outcome = successful_action_outcome("hid app");
                apply_prepared_target(&mut outcome, Some(&prepared));
                outcome.side_effects = vec![ActionSideEffect::HideApp];
                Ok(outcome)
            }
            Action::UnhideApp => {
                let prepared = self.lifecycle_action_target(target)?;
                let app_name = prepared.app_name()?;
                self.app_service.unhide_app(&app_name)?;
                let mut outcome = successful_action_outcome("unhid app");
                apply_prepared_target(&mut outcome, Some(&prepared));
                outcome.side_effects = vec![ActionSideEffect::UnhideApp];
                Ok(outcome)
            }
            Action::Click { mode } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.click(locator, mode, target, &permissions)
            }
            Action::Move => {
                let permissions = self.permission_reader.current_permissions()?;
                self.move_pointer(locator, target, &permissions)
            }
            Action::Type {
                text,
                clear_before,
                delay_ms,
                trailing_keys,
            } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.type_text(
                    locator,
                    TypeActionConfig {
                        text: &text,
                        clear_before,
                        delay_ms,
                        trailing_keys: &trailing_keys,
                        target,
                    },
                    &permissions,
                )
            }
            Action::Scroll { delta_x, delta_y } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.scroll(locator, delta_x, delta_y, target, &permissions)
            }
            Action::Drag { from, to, motion } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.drag(from, to, motion, target, &permissions)
            }
            Action::Swipe {
                from,
                to,
                duration_ms,
                steps,
            } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.swipe(from, to, duration_ms, steps, target, &permissions)
            }
            Action::Hotkey { keys } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.hotkey(&keys, target, &permissions)
            }
            Action::Press { key, count } => {
                let permissions = self.permission_reader.current_permissions()?;
                self.press(&key, count.get(), None, target, &permissions)
            }
            Action::FocusWindow { id } => {
                let window = self.resolve_window_by_id(id)?;
                self.app_service.focus_window(id)?;
                let mut outcome = successful_action_outcome(format!("focused window {id}"));
                outcome.target_window = Some(window_with_focus(&window, true));
                outcome.side_effects = vec![ActionSideEffect::FocusWindow];
                Ok(outcome)
            }
        }
    }
}

fn filter_app_infos(apps: Vec<AppInfo>, filter: &AppListFilter) -> Vec<AppInfo> {
    apps.into_iter().filter(|app| filter.matches(app)).collect()
}

impl<A, P, C, I, S> MacosDriver<A, P, C, I, S>
where
    A: AppService,
    P: PermissionReader,
    I: TreeInspector,
    S: InputSynthesizer,
{
    fn click(
        &self,
        locator: Option<Locator>,
        mode: ClickMode,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            if locator.is_some() {
                AnchorWindowResolution::Optional
            } else {
                AnchorWindowResolution::Required
            },
        )?;
        let (point, warning) = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            (Some(resolved.point), resolved.warning)
        } else if let Some(target) = prepared.as_ref() {
            (Some(target_pointer_point(target)?), None)
        } else {
            (None, None)
        };
        self.input_synthesizer.click(point, mode)?;
        let _ = self.effects.on_click(point, mode);
        let mut outcome =
            successful_action_outcome(action_detail(click_detail(mode), warning.as_deref()));
        outcome.coordinates = Some(ActionCoordinates {
            point,
            from: None,
            to: None,
        });
        apply_prepared_target(&mut outcome, prepared.as_ref());
        apply_warning(&mut outcome, warning);
        outcome.side_effects = vec![ActionSideEffect::Click { mode }];
        Ok(outcome)
    }

    fn type_text(
        &self,
        locator: Option<Locator>,
        input: TypeActionConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            input.target.selector,
            input.target.focus_policy,
            AnchorWindowResolution::Optional,
        )?;
        let (point, warning) = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            self.input_synthesizer
                .click(Some(resolved.point), ClickMode::Left)?;
            thread::sleep(std::time::Duration::from_millis(50));
            (Some(resolved.point), resolved.warning)
        } else {
            (None, None)
        };
        if input.clear_before {
            let clear_keys = vec!["command".to_string(), "a".to_string()];
            self.input_synthesizer.hotkey(&clear_keys)?;
            self.input_synthesizer.press("delete", 1, input.delay_ms)?;
        }
        self.input_synthesizer
            .type_text(input.text, input.delay_ms)?;
        for key in input.trailing_keys {
            self.input_synthesizer
                .press(type_trailing_key_name(*key), 1, input.delay_ms)?;
        }
        let keyboard_label = type_effect_label(input.text, input.trailing_keys);
        let _ = self.effects.on_keyboard(&keyboard_label);
        let mut outcome = successful_action_outcome(action_detail(
            &type_detail(input.clear_before, input.trailing_keys),
            warning.as_deref(),
        ));
        outcome.coordinates = Some(ActionCoordinates {
            point,
            from: None,
            to: None,
        });
        apply_prepared_target(&mut outcome, prepared.as_ref());
        apply_warning(&mut outcome, warning);
        outcome.side_effects = vec![ActionSideEffect::Type {
            clear_before: input.clear_before,
            trailing_keys: input.trailing_keys.to_vec(),
        }];
        Ok(outcome)
    }

    fn move_pointer(
        &self,
        locator: Option<Locator>,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            if locator.is_some() {
                AnchorWindowResolution::Optional
            } else {
                AnchorWindowResolution::Required
            },
        )?;
        let (point, warning) = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            (resolved.point, resolved.warning)
        } else if let Some(target) = prepared.as_ref() {
            (target_pointer_point(target)?, None)
        } else {
            return Err(OperatorError::Platform(
                "move requires a locator or target selector".into(),
            ));
        };
        self.input_synthesizer.move_pointer(point)?;
        let _ = self.effects.on_move(point);
        let mut outcome = successful_action_outcome(action_detail("moved", warning.as_deref()));
        outcome.coordinates = Some(ActionCoordinates {
            point: Some(point),
            from: None,
            to: None,
        });
        apply_prepared_target(&mut outcome, prepared.as_ref());
        apply_warning(&mut outcome, warning);
        outcome.side_effects = vec![ActionSideEffect::MoveCursor];
        Ok(outcome)
    }

    fn scroll(
        &self,
        locator: Option<Locator>,
        delta_x: f64,
        delta_y: f64,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            if locator.is_some() {
                AnchorWindowResolution::Optional
            } else {
                AnchorWindowResolution::Required
            },
        )?;
        let (point, warning) = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            (Some(resolved.point), resolved.warning)
        } else if let Some(target) = prepared.as_ref() {
            (Some(target_pointer_point(target)?), None)
        } else {
            (None, None)
        };
        self.input_synthesizer.scroll(point, delta_x, delta_y)?;
        let _ = self.effects.on_scroll(point, delta_x, delta_y);
        let mut outcome = successful_action_outcome(action_detail("scrolled", warning.as_deref()));
        outcome.coordinates = Some(ActionCoordinates {
            point,
            from: None,
            to: None,
        });
        apply_prepared_target(&mut outcome, prepared.as_ref());
        apply_warning(&mut outcome, warning);
        outcome.side_effects = vec![ActionSideEffect::Scroll { delta_x, delta_y }];
        Ok(outcome)
    }

    fn drag(
        &self,
        from: Locator,
        to: Locator,
        motion: DragMotion,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            AnchorWindowResolution::Optional,
        )?;
        let from = resolve_locator(&from, &self.tree_inspector)?;
        let to = resolve_locator(&to, &self.tree_inspector)?;
        self.input_synthesizer.drag(from.point, to.point, &motion)?;
        let _ = self.effects.on_drag(from.point, to.point);
        let mut outcome = successful_action_outcome("dragged");
        outcome.coordinates = Some(ActionCoordinates {
            point: None,
            from: Some(from.point),
            to: Some(to.point),
        });
        apply_prepared_target(&mut outcome, prepared.as_ref());
        apply_warning(&mut outcome, from.warning);
        apply_warning(&mut outcome, to.warning);
        outcome.side_effects = vec![ActionSideEffect::Drag { motion }];
        Ok(outcome)
    }

    fn swipe(
        &self,
        from: Locator,
        to: Locator,
        duration_ms: Option<u64>,
        steps: Option<std::num::NonZeroU32>,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            AnchorWindowResolution::Optional,
        )?;
        let from = resolve_locator(&from, &self.tree_inspector)?;
        let to = resolve_locator(&to, &self.tree_inspector)?;
        self.input_synthesizer
            .swipe(from.point, to.point, duration_ms, steps)?;
        let _ = self.effects.on_drag(from.point, to.point);
        let mut outcome = successful_action_outcome("swiped");
        outcome.coordinates = Some(ActionCoordinates {
            point: None,
            from: Some(from.point),
            to: Some(to.point),
        });
        apply_prepared_target(&mut outcome, prepared.as_ref());
        apply_warning(&mut outcome, from.warning);
        apply_warning(&mut outcome, to.warning);
        outcome.side_effects = vec![ActionSideEffect::Swipe { duration_ms, steps }];
        Ok(outcome)
    }

    fn hotkey(
        &self,
        keys: &[String],
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            AnchorWindowResolution::Optional,
        )?;
        self.input_synthesizer.hotkey(keys)?;
        let keyboard_label = hotkey_effect_label(keys);
        let _ = self.effects.on_keyboard(&keyboard_label);
        let mut outcome = successful_action_outcome("sent hotkey");
        apply_prepared_target(&mut outcome, prepared.as_ref());
        outcome.side_effects = vec![ActionSideEffect::Hotkey {
            keys: keys.to_vec(),
        }];
        Ok(outcome)
    }

    fn press(
        &self,
        key: &str,
        count: u32,
        delay_ms: Option<u64>,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            AnchorWindowResolution::Optional,
        )?;
        self.input_synthesizer.press(key, count, delay_ms)?;
        let keyboard_label = press_effect_label(key, count);
        let _ = self.effects.on_keyboard(&keyboard_label);
        let mut outcome = successful_action_outcome(press_detail(key, count));
        apply_prepared_target(&mut outcome, prepared.as_ref());
        outcome.side_effects = vec![ActionSideEffect::Press {
            key: key.to_string(),
            count,
        }];
        Ok(outcome)
    }

    fn prepare_action_target(
        &self,
        selector: Option<&ActionTargetSelector>,
        focus_policy: ActionFocusPolicy,
        anchor_window: AnchorWindowResolution,
    ) -> Result<Option<PreparedActionTarget>, OperatorError> {
        let mut prepared = selector
            .map(|selector| self.resolve_action_target(selector, anchor_window))
            .transpose()?;

        if matches!(focus_policy, ActionFocusPolicy::Auto) {
            if let Some(target) = prepared.as_ref() {
                target.focus(&self.app_service)?;
                thread::sleep(std::time::Duration::from_millis(50));
            }
            if let Some(target) = prepared.as_mut() {
                self.refresh_prepared_target_window(target, anchor_window)?;
            }
        }

        Ok(prepared)
    }

    fn resolve_action_target(
        &self,
        selector: &ActionTargetSelector,
        anchor_window: AnchorWindowResolution,
    ) -> Result<PreparedActionTarget, OperatorError> {
        match selector {
            ActionTargetSelector::App(bundle_id_or_name) => {
                let app = self.resolve_app_by_identity(bundle_id_or_name)?;
                Ok(PreparedActionTarget::App(PreparedAppTarget {
                    anchor_window: self.resolve_app_anchor_window(&app, anchor_window)?,
                    app,
                }))
            }
            ActionTargetSelector::Pid(pid) => {
                let app = self.resolve_app_by_pid(*pid)?;
                Ok(PreparedActionTarget::App(PreparedAppTarget {
                    anchor_window: self.resolve_app_anchor_window(&app, anchor_window)?,
                    app,
                }))
            }
            ActionTargetSelector::WindowId(id) => self
                .resolve_window_by_id(*id)
                .map(PreparedActionTarget::Window),
            ActionTargetSelector::WindowTitle(title) => self
                .resolve_window_by_title(title)
                .map(PreparedActionTarget::Window),
            ActionTargetSelector::WindowIndex(index) => self
                .resolve_window_by_index(*index)
                .map(PreparedActionTarget::Window),
        }
    }

    fn resolve_app_by_identity(&self, bundle_id_or_name: &str) -> Result<AppInfo, OperatorError> {
        let apps = self.app_service.list_apps(AppListMode::Running)?;
        let matches =
            apps.into_iter()
                .filter(|app| {
                    app.name.eq_ignore_ascii_case(bundle_id_or_name)
                        || app.bundle_id.as_deref().is_some_and(|bundle_id| {
                            bundle_id.eq_ignore_ascii_case(bundle_id_or_name)
                        })
                })
                .collect::<Vec<_>>();

        select_single_app(
            matches,
            &format!("macOS action target app not found: {bundle_id_or_name}"),
            &format!("macOS action target app is ambiguous: {bundle_id_or_name}"),
        )
    }

    fn resolve_app_by_pid(&self, pid: u32) -> Result<AppInfo, OperatorError> {
        let apps = self.app_service.list_apps(AppListMode::Running)?;
        let matches = apps
            .into_iter()
            .filter(|app| app.pid == Some(pid))
            .collect::<Vec<_>>();

        select_single_app(
            matches,
            &format!("macOS action target pid not found: {pid}"),
            &format!("macOS action target pid is ambiguous: {pid}"),
        )
    }

    fn resolve_anchor_window(&self, app: &AppInfo) -> Result<Option<WindowInfo>, OperatorError> {
        let windows = self.app_service.list_windows(Some(&app.name))?;
        if let Some(window) = select_anchor_window(&windows) {
            return Ok(Some(window));
        }

        let focus = self.app_service.get_focus()?;
        if focus_matches_expected_app(focus.as_ref(), app) {
            let windows = self.app_service.list_frontmost_windows()?;
            if let Some(window) = select_matching_app_window(&windows, app, focus.as_ref()) {
                return Ok(Some(window));
            }
            if let Some(window) = select_observe_app_window(&windows, app, focus.as_ref()) {
                return Ok(Some(window));
            }
        }

        Ok(None)
    }

    fn resolve_observe_surface(&self, surface: &Surface) -> ResolvedObserveSurface {
        match &surface.kind {
            SurfaceKind::Frontmost => self.resolve_frontmost_observe_surface(),
            _ => ResolvedObserveSurface::new(surface.clone(), surface.clone(), None),
        }
    }

    fn resolve_frontmost_observe_surface(&self) -> ResolvedObserveSurface {
        if let Ok(windows) = self.app_service.list_frontmost_window_targets() {
            if let Some(window) = select_observe_window_target(&windows) {
                return ResolvedObserveSurface::window_target(window);
            }
        }

        ResolvedObserveSurface::new(
            Surface {
                kind: SurfaceKind::Frontmost,
            },
            Surface {
                kind: SurfaceKind::Frontmost,
            },
            None,
        )
    }

    fn resolve_app_anchor_window(
        &self,
        app: &AppInfo,
        anchor_window: AnchorWindowResolution,
    ) -> Result<Option<WindowInfo>, OperatorError> {
        match self.resolve_anchor_window(app) {
            Ok(window) => Ok(window),
            Err(_) if matches!(anchor_window, AnchorWindowResolution::Optional) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn resolve_window_by_id(
        &self,
        id: operator_core::WindowId,
    ) -> Result<WindowInfo, OperatorError> {
        let windows = self.app_service.list_windows(None)?;
        windows
            .into_iter()
            .find(|window| window.id == id)
            .ok_or_else(|| {
                OperatorError::Platform(format!("macOS action target window not found: {id}"))
            })
    }

    fn resolve_window_by_title(&self, title: &str) -> Result<WindowInfo, OperatorError> {
        let windows = self.app_service.list_windows(None)?;
        let matches = windows
            .into_iter()
            .filter(|window| {
                window
                    .title
                    .as_deref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(title))
            })
            .collect::<Vec<_>>();

        select_single_window(
            matches,
            &format!("macOS action target window title not found: {title}"),
            &format!("macOS action target window title is ambiguous: {title}"),
        )
    }

    fn resolve_window_by_index(&self, index: usize) -> Result<WindowInfo, OperatorError> {
        let windows = self.app_service.list_windows(None)?;
        windows.get(index).cloned().ok_or_else(|| {
            OperatorError::Platform(format!(
                "macOS action target window index not found: {index}"
            ))
        })
    }

    fn lifecycle_action_target(
        &self,
        target: ActionTargetConfig<'_>,
    ) -> Result<PreparedActionTarget, OperatorError> {
        let selector = target.selector.ok_or_else(|| {
            OperatorError::Platform("app lifecycle actions require a target selector".into())
        })?;

        self.resolve_action_target(selector, AnchorWindowResolution::Optional)
    }

    fn window_action_target(
        &self,
        target: ActionTargetConfig<'_>,
        action_name: &str,
    ) -> Result<WindowInfo, OperatorError> {
        let prepared = self.prepare_action_target(
            target.selector,
            target.focus_policy,
            AnchorWindowResolution::Required,
        )?;
        let prepared = prepared.ok_or_else(|| {
            OperatorError::Platform(format!("{action_name} requires a target selector"))
        })?;
        prepared.window()
    }

    fn refresh_prepared_target_window(
        &self,
        prepared: &mut PreparedActionTarget,
        anchor_window: AnchorWindowResolution,
    ) -> Result<(), OperatorError> {
        let PreparedActionTarget::App(target) = prepared else {
            return Ok(());
        };
        if target.anchor_window.is_some() {
            return Ok(());
        }
        target.anchor_window = self.resolve_app_anchor_window(&target.app, anchor_window)?;
        if target.anchor_window.is_none() {
            target.anchor_window = self.resolve_frontmost_observe_window();
        }
        Ok(())
    }

    fn resolve_frontmost_observe_window(&self) -> Option<WindowInfo> {
        self.app_service
            .list_frontmost_windows()
            .ok()
            .and_then(|windows| select_observe_window(&windows))
    }
}

#[derive(Debug, Clone)]
enum PreparedActionTarget {
    App(PreparedAppTarget),
    Window(WindowInfo),
}

impl PreparedActionTarget {
    fn focus<A: AppService>(&self, app_service: &A) -> Result<(), OperatorError> {
        match self {
            Self::App(target) => app_service.focus_app(&app_focus_identity(&target.app)),
            Self::Window(window) => app_service.focus_window(window.id),
        }
    }

    fn app(&self) -> Option<&AppInfo> {
        match self {
            Self::App(target) => Some(&target.app),
            Self::Window(_) => None,
        }
    }

    fn target_window(&self) -> Option<&WindowInfo> {
        match self {
            Self::App(target) => target.anchor_window.as_ref(),
            Self::Window(window) => Some(window),
        }
    }

    fn app_name(&self) -> Result<String, OperatorError> {
        match self {
            Self::App(target) => Ok(target.app.name.clone()),
            Self::Window(window) => window_app_name(window.clone()),
        }
    }

    fn window(&self) -> Result<WindowInfo, OperatorError> {
        match self {
            Self::App(target) => target.anchor_window.clone().ok_or_else(|| {
                OperatorError::Platform(format!(
                    "macOS action target app has no windows: {}",
                    target.app.name
                ))
            }),
            Self::Window(window) => Ok(window.clone()),
        }
    }
}

#[derive(Debug, Clone)]
struct PreparedAppTarget {
    app: AppInfo,
    anchor_window: Option<WindowInfo>,
}

#[derive(Debug, Clone, Copy)]
struct ActionTargetConfig<'a> {
    selector: Option<&'a ActionTargetSelector>,
    focus_policy: ActionFocusPolicy,
}

impl<'a> ActionTargetConfig<'a> {
    fn new(selector: Option<&'a ActionTargetSelector>, focus_policy: ActionFocusPolicy) -> Self {
        Self {
            selector,
            focus_policy,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TypeActionConfig<'a> {
    text: &'a str,
    clear_before: bool,
    delay_ms: Option<u64>,
    trailing_keys: &'a [TypeTrailingKey],
    target: ActionTargetConfig<'a>,
}

#[derive(Debug, Clone)]
struct ResolvedObserveSurface {
    capture_surface: Surface,
    inspect_surface: Surface,
    capture_bounds: Option<Rect>,
    window_target: Option<WindowTarget>,
}

impl ResolvedObserveSurface {
    fn new(
        capture_surface: Surface,
        inspect_surface: Surface,
        capture_bounds: Option<Rect>,
    ) -> Self {
        Self {
            capture_surface,
            inspect_surface,
            capture_bounds,
            window_target: None,
        }
    }

    fn window_target(window_target: WindowTarget) -> Self {
        let inspect_surface = Surface {
            kind: SurfaceKind::Window {
                id: window_target.window.id,
            },
        };
        let capture_surface = if window_target.native_id.is_some() {
            Surface {
                kind: SurfaceKind::Window {
                    id: window_target.window.id,
                },
            }
        } else {
            window_target
                .window
                .bounds
                .map(|bounds| Surface {
                    kind: SurfaceKind::Region { rect: bounds },
                })
                .unwrap_or(inspect_surface.clone())
        };
        Self {
            capture_surface,
            inspect_surface,
            capture_bounds: window_target.window.bounds,
            window_target: Some(window_target),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AnchorWindowResolution {
    Optional,
    Required,
}

fn successful_action_outcome(detail: impl Into<String>) -> ActionOutcome {
    ActionOutcome {
        success: true,
        duration_ms: 0,
        detail: Some(detail.into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }
}

fn apply_prepared_target(outcome: &mut ActionOutcome, prepared: Option<&PreparedActionTarget>) {
    if let Some(prepared) = prepared {
        outcome.target_app = prepared.app().cloned();
        outcome.target_window = prepared.target_window().cloned();
    }
}

fn apply_warning(outcome: &mut ActionOutcome, warning: Option<String>) {
    if let Some(warning) = warning {
        outcome.warnings.push(warning);
    }
}

fn window_with_bounds(window: &WindowInfo, bounds: Rect) -> WindowInfo {
    let mut updated = window.clone();
    updated.bounds = Some(bounds);
    updated
}

fn window_with_minimized(window: &WindowInfo, is_minimized: bool) -> WindowInfo {
    let mut updated = window.clone();
    updated.is_minimized = is_minimized;
    updated
}

fn window_with_focus(window: &WindowInfo, is_focused: bool) -> WindowInfo {
    let mut updated = window.clone();
    updated.is_focused = is_focused;
    updated
}

fn select_single_app(
    matches: Vec<AppInfo>,
    not_found: &str,
    ambiguous: &str,
) -> Result<AppInfo, OperatorError> {
    match matches.len() {
        0 => Err(OperatorError::Platform(not_found.into())),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => Err(OperatorError::Platform(ambiguous.into())),
    }
}

fn select_single_window(
    matches: Vec<WindowInfo>,
    not_found: &str,
    ambiguous: &str,
) -> Result<WindowInfo, OperatorError> {
    match matches.len() {
        0 => Err(OperatorError::Platform(not_found.into())),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => Err(OperatorError::Platform(ambiguous.into())),
    }
}

fn select_anchor_window(windows: &[WindowInfo]) -> Option<WindowInfo> {
    windows
        .iter()
        .find(|window| window.is_focused)
        .cloned()
        .or_else(|| windows.iter().find(|window| !window.is_minimized).cloned())
        .or_else(|| windows.first().cloned())
}

fn select_observe_window(windows: &[WindowInfo]) -> Option<WindowInfo> {
    windows
        .iter()
        .filter(|window| window_has_usable_observe_bounds(window))
        .find(|window| window.is_focused && !window.is_minimized)
        .cloned()
        .or_else(|| {
            windows
                .iter()
                .filter(|window| window_has_usable_observe_bounds(window))
                .find(|window| !window.is_minimized)
                .cloned()
        })
        .or_else(|| select_anchor_window(windows))
}

fn select_observe_window_target(windows: &[WindowTarget]) -> Option<WindowTarget> {
    windows
        .iter()
        .filter(|window| window_has_usable_observe_bounds(&window.window))
        .find(|window| window.window.is_focused && !window.window.is_minimized)
        .cloned()
        .or_else(|| {
            windows
                .iter()
                .filter(|window| window_has_usable_observe_bounds(&window.window))
                .find(|window| !window.window.is_minimized)
                .cloned()
        })
        .or_else(|| {
            windows
                .iter()
                .find(|window| window.window.is_focused)
                .cloned()
                .or_else(|| {
                    windows
                        .iter()
                        .find(|window| !window.window.is_minimized)
                        .cloned()
                })
                .or_else(|| windows.first().cloned())
        })
}

fn window_has_usable_observe_bounds(window: &WindowInfo) -> bool {
    window
        .bounds
        .is_some_and(|bounds| bounds.width >= 80.0 && bounds.height >= 80.0)
}

fn select_matching_app_window(
    windows: &[WindowInfo],
    app: &AppInfo,
    focus: Option<&operator_core::FocusInfo>,
) -> Option<WindowInfo> {
    let matching = windows
        .iter()
        .filter(|window| window_matches_expected_app(window, app, focus))
        .cloned()
        .collect::<Vec<_>>();

    select_anchor_window(&matching)
}

fn select_observe_app_window(
    windows: &[WindowInfo],
    app: &AppInfo,
    focus: Option<&operator_core::FocusInfo>,
) -> Option<WindowInfo> {
    let matching = windows
        .iter()
        .filter(|window| window_matches_expected_app(window, app, focus))
        .cloned()
        .collect::<Vec<_>>();

    select_observe_window(&matching)
}

fn window_matches_expected_app(
    window: &WindowInfo,
    app: &AppInfo,
    focus: Option<&operator_core::FocusInfo>,
) -> bool {
    let Some(actual_name) = window.app_name.as_deref() else {
        return false;
    };

    if actual_name.eq_ignore_ascii_case(&app.name) {
        return true;
    }

    focus
        .and_then(|focus| focus.app_name.as_deref())
        .is_some_and(|focus_name| actual_name.eq_ignore_ascii_case(focus_name))
}

fn focus_matches_expected_app(focus: Option<&operator_core::FocusInfo>, app: &AppInfo) -> bool {
    let Some(focus) = focus else {
        return false;
    };

    if let (Some(actual_bundle_id), Some(expected_bundle_id)) =
        (focus.bundle_id.as_deref(), app.bundle_id.as_deref())
    {
        if actual_bundle_id.eq_ignore_ascii_case(expected_bundle_id) {
            return true;
        }
    }

    focus
        .app_name
        .as_deref()
        .is_some_and(|actual_name| actual_name.eq_ignore_ascii_case(&app.name))
}

fn window_app_name(window: WindowInfo) -> Result<String, OperatorError> {
    window.app_name.ok_or_else(|| {
        OperatorError::Platform(format!(
            "macOS action target window has no owning app metadata: {}",
            window.id
        ))
    })
}

fn target_pointer_point(target: &PreparedActionTarget) -> Result<Point, OperatorError> {
    let point = match target {
        PreparedActionTarget::App(target) => target.anchor_window.as_ref().and_then(window_center),
        PreparedActionTarget::Window(window) => window_center(window),
    };

    point.ok_or_else(|| {
        OperatorError::Platform(
            "macOS action target does not resolve to a window with bounds".into(),
        )
    })
}

fn window_center(window: &WindowInfo) -> Option<Point> {
    window.bounds.map(|bounds| Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    })
}

fn app_focus_identity(app: &AppInfo) -> String {
    app.bundle_id.clone().unwrap_or_else(|| app.name.clone())
}

fn require_observe_permissions(
    permissions: &operator_core::PermissionsReport,
    req: &ObserveRequest,
) -> Result<(), OperatorError> {
    if req.include_screenshot
        && permission_status(permissions, SCREEN_RECORDING_CHECK_ID) != PermissionStatus::Granted
    {
        return Err(OperatorError::PermissionDenied(
            "Screen Recording permission is required for macOS capture.".into(),
        ));
    }

    if req.include_elements
        && permission_status(permissions, ACCESSIBILITY_CHECK_ID) != PermissionStatus::Granted
    {
        return Err(OperatorError::PermissionDenied(
            "Accessibility permission is required for macOS tree inspection.".into(),
        ));
    }

    Ok(())
}

fn require_accessibility_permission(
    permissions: &operator_core::PermissionsReport,
) -> Result<(), OperatorError> {
    if permission_status(permissions, ACCESSIBILITY_CHECK_ID) != PermissionStatus::Granted {
        return Err(OperatorError::PermissionDenied(
            "Accessibility permission is required for macOS input.".into(),
        ));
    }

    Ok(())
}

fn require_system_events_permission(
    permissions: &operator_core::PermissionsReport,
) -> Result<(), OperatorError> {
    if permission_status(permissions, SYSTEM_EVENTS_CHECK_ID) != PermissionStatus::Granted {
        return Err(OperatorError::PermissionDenied(
            "System Events access is required for macOS window queries and focus reads.".into(),
        ));
    }

    Ok(())
}

fn permission_status(permissions: &operator_core::PermissionsReport, id: &str) -> PermissionStatus {
    permissions
        .status(id)
        .unwrap_or(PermissionStatus::NotDetermined)
}

fn action_detail(action: &str, warning: Option<&str>) -> String {
    match warning {
        Some(warning) => format!("{action}; {warning}"),
        None => action.to_string(),
    }
}

fn click_detail(mode: ClickMode) -> &'static str {
    match mode {
        ClickMode::Left => "clicked",
        ClickMode::Right => "right-clicked",
        ClickMode::Middle => "middle-clicked",
        ClickMode::Double => "double-clicked",
    }
}

fn press_detail(key: &str, count: u32) -> String {
    if count == 1 {
        format!("pressed {key}")
    } else {
        format!("pressed {key} {count} times")
    }
}

fn press_effect_label(key: &str, count: u32) -> String {
    if count == 1 {
        key.to_string()
    } else {
        format!("{key} x{count}")
    }
}

fn window_geometry_detail(action: &str, id: operator_core::WindowId, bounds: Rect) -> String {
    match action {
        "set" => format!(
            "set window {id} bounds to x={} y={} width={} height={}",
            trim_trailing_zero(bounds.x),
            trim_trailing_zero(bounds.y),
            trim_trailing_zero(bounds.width),
            trim_trailing_zero(bounds.height)
        ),
        _ => format!(
            "{action} window {id} to x={} y={} width={} height={}",
            trim_trailing_zero(bounds.x),
            trim_trailing_zero(bounds.y),
            trim_trailing_zero(bounds.width),
            trim_trailing_zero(bounds.height)
        ),
    }
}

fn trim_trailing_zero(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn type_detail(clear_before: bool, trailing_keys: &[TypeTrailingKey]) -> String {
    let mut detail = if clear_before {
        "cleared and typed text".to_string()
    } else {
        "typed text".to_string()
    };

    if !trailing_keys.is_empty() {
        let trailing = trailing_keys
            .iter()
            .map(|key| type_trailing_key_name(*key))
            .collect::<Vec<_>>()
            .join(" and ");
        detail.push_str(", then ");
        detail.push_str(&trailing);
    }

    detail
}

fn type_effect_label(text: &str, trailing_keys: &[TypeTrailingKey]) -> String {
    let mut parts = Vec::new();

    if !text.is_empty() {
        parts.push(text.to_string());
    }

    if !trailing_keys.is_empty() {
        parts.push(
            trailing_keys
                .iter()
                .map(|key| type_trailing_key_name(*key).to_string())
                .collect::<Vec<_>>()
                .join("+"),
        );
    }

    if parts.is_empty() {
        "type".to_string()
    } else {
        parts.join(" + ")
    }
}

fn hotkey_effect_label(keys: &[String]) -> String {
    if keys.is_empty() {
        "hotkey".to_string()
    } else {
        keys.join("+")
    }
}

fn type_trailing_key_name(key: TypeTrailingKey) -> &'static str {
    match key {
        TypeTrailingKey::Return => "return",
        TypeTrailingKey::Tab => "tab",
        TypeTrailingKey::Escape => "escape",
        TypeTrailingKey::Delete => "delete",
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
