use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use operator_core::{
    Action, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionTargetSelector, AppInfo,
    Capability, CapabilitySet, ClickMode, DragMotion, ExecContext, HealthStatus, Locator,
    ObserveRequest, ObserveResult, OperatorError, PermissionStatus, Point, QueryRequest,
    QueryResult, Snapshot, SnapshotMetadata, TypeTrailingKey, WindowInfo,
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
        let ActionRequest {
            action,
            locator,
            target_selector,
            focus_policy,
        } = req;
        let target = ActionTargetConfig::new(target_selector.as_ref(), focus_policy);

        match action {
            Action::LaunchApp { bundle_id_or_name } => {
                self.app_service.launch_app(&bundle_id_or_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some(format!("launched {bundle_id_or_name}")),
                })
            }
            Action::CloseWindow => {
                let window = self.window_action_target(target, "close-window")?;
                self.app_service.close_window(window.id)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some(format!("closed window {}", window.id)),
                })
            }
            Action::MinimizeWindow => {
                let window = self.window_action_target(target, "minimize-window")?;
                self.app_service.minimize_window(window.id)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some(format!("minimized window {}", window.id)),
                })
            }
            Action::MaximizeWindow => {
                let window = self.window_action_target(target, "maximize-window")?;
                self.app_service.maximize_window(window.id)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some(format!("maximized window {}", window.id)),
                })
            }
            Action::SwitchApp => {
                let app_name = self.lifecycle_target_name(target.selector)?;
                self.app_service.focus_app(&app_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some("switched app".into()),
                })
            }
            Action::QuitApp => {
                let app_name = self.lifecycle_target_name(target.selector)?;
                self.app_service.quit_app(&app_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some("quit app".into()),
                })
            }
            Action::RelaunchApp => {
                let app_name = self.lifecycle_target_name(target.selector)?;
                self.app_service.relaunch_app(&app_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some("relaunched app".into()),
                })
            }
            Action::HideApp => {
                let app_name = self.lifecycle_target_name(target.selector)?;
                self.app_service.hide_app(&app_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some("hid app".into()),
                })
            }
            Action::UnhideApp => {
                let app_name = self.lifecycle_target_name(target.selector)?;
                self.app_service.unhide_app(&app_name)?;
                Ok(ActionOutcome {
                    success: true,
                    duration_ms: 0,
                    detail: Some("unhid app".into()),
                })
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
        let prepared = self.prepare_action_target(target.selector, target.focus_policy)?;
        let (point, warning) = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            (Some(resolved.point), resolved.warning)
        } else if let Some(target) = prepared.as_ref() {
            (Some(target_pointer_point(target)?), None)
        } else {
            (None, None)
        };
        self.input_synthesizer.click(point, mode)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(action_detail(click_detail(mode), warning.as_deref())),
        })
    }

    fn type_text(
        &self,
        locator: Option<Locator>,
        input: TypeActionConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        self.prepare_action_target(input.target.selector, input.target.focus_policy)?;
        let warning = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            self.input_synthesizer
                .click(Some(resolved.point), ClickMode::Left)?;
            thread::sleep(std::time::Duration::from_millis(50));
            resolved.warning
        } else {
            None
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
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(action_detail(
                &type_detail(input.clear_before, input.trailing_keys),
                warning.as_deref(),
            )),
        })
    }

    fn move_pointer(
        &self,
        locator: Option<Locator>,
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        let prepared = self.prepare_action_target(target.selector, target.focus_policy)?;
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
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(action_detail("moved", warning.as_deref())),
        })
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
        let prepared = self.prepare_action_target(target.selector, target.focus_policy)?;
        let (point, warning) = if let Some(locator) = locator {
            let resolved = resolve_locator(&locator, &self.tree_inspector)?;
            (Some(resolved.point), resolved.warning)
        } else if let Some(target) = prepared.as_ref() {
            (Some(target_pointer_point(target)?), None)
        } else {
            (None, None)
        };
        self.input_synthesizer.scroll(point, delta_x, delta_y)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(action_detail("scrolled", warning.as_deref())),
        })
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
        self.prepare_action_target(target.selector, target.focus_policy)?;
        let from = resolve_locator(&from, &self.tree_inspector)?;
        let to = resolve_locator(&to, &self.tree_inspector)?;
        self.input_synthesizer.drag(from.point, to.point, &motion)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some("dragged".into()),
        })
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
        self.prepare_action_target(target.selector, target.focus_policy)?;
        let from = resolve_locator(&from, &self.tree_inspector)?;
        let to = resolve_locator(&to, &self.tree_inspector)?;
        self.input_synthesizer
            .swipe(from.point, to.point, duration_ms, steps)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some("swiped".into()),
        })
    }

    fn hotkey(
        &self,
        keys: &[String],
        target: ActionTargetConfig<'_>,
        permissions: &operator_core::PermissionsReport,
    ) -> Result<ActionOutcome, OperatorError> {
        require_accessibility_permission(permissions)?;
        self.prepare_action_target(target.selector, target.focus_policy)?;
        self.input_synthesizer.hotkey(keys)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some("sent hotkey".into()),
        })
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
        self.prepare_action_target(target.selector, target.focus_policy)?;
        self.input_synthesizer.press(key, count, delay_ms)?;
        Ok(ActionOutcome {
            success: true,
            duration_ms: 0,
            detail: Some(press_detail(key, count)),
        })
    }

    fn prepare_action_target(
        &self,
        selector: Option<&ActionTargetSelector>,
        focus_policy: ActionFocusPolicy,
    ) -> Result<Option<PreparedActionTarget>, OperatorError> {
        let prepared = selector
            .map(|selector| self.resolve_action_target(selector))
            .transpose()?;

        if matches!(focus_policy, ActionFocusPolicy::Auto) {
            if let Some(target) = prepared.as_ref() {
                target.focus(&self.app_service)?;
                thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        Ok(prepared)
    }

    fn resolve_action_target(
        &self,
        selector: &ActionTargetSelector,
    ) -> Result<PreparedActionTarget, OperatorError> {
        match selector {
            ActionTargetSelector::App(bundle_id_or_name) => {
                let app = self.resolve_app_by_identity(bundle_id_or_name)?;
                Ok(PreparedActionTarget::App(PreparedAppTarget {
                    anchor_window: self.resolve_anchor_window(&app)?,
                    app,
                }))
            }
            ActionTargetSelector::Pid(pid) => {
                let app = self.resolve_app_by_pid(*pid)?;
                Ok(PreparedActionTarget::App(PreparedAppTarget {
                    anchor_window: self.resolve_anchor_window(&app)?,
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
        let apps = self.app_service.list_apps()?;
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
        let apps = self.app_service.list_apps()?;
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
        Ok(select_anchor_window(&windows))
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

    fn lifecycle_target_name(
        &self,
        selector: Option<&ActionTargetSelector>,
    ) -> Result<String, OperatorError> {
        let selector = selector.ok_or_else(|| {
            OperatorError::Platform("app lifecycle actions require a target selector".into())
        })?;

        match selector {
            ActionTargetSelector::App(bundle_id_or_name) => self
                .resolve_app_by_identity(bundle_id_or_name)
                .map(|app| app.name),
            ActionTargetSelector::Pid(pid) => self.resolve_app_by_pid(*pid).map(|app| app.name),
            ActionTargetSelector::WindowId(id) => {
                self.resolve_window_by_id(*id).and_then(window_app_name)
            }
            ActionTargetSelector::WindowTitle(title) => self
                .resolve_window_by_title(title)
                .and_then(window_app_name),
            ActionTargetSelector::WindowIndex(index) => self
                .resolve_window_by_index(*index)
                .and_then(window_app_name),
        }
    }

    fn window_action_target(
        &self,
        target: ActionTargetConfig<'_>,
        action_name: &str,
    ) -> Result<WindowInfo, OperatorError> {
        let prepared = self.prepare_action_target(target.selector, target.focus_policy)?;
        let prepared = prepared.ok_or_else(|| {
            OperatorError::Platform(format!("{action_name} requires a target selector"))
        })?;
        prepared.window()
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
