use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::Instant,
};

use hmdriver_rs::{CorrelatedWindowList, CurrentApp, Driver, ShellResult, UiDriver};
use operator_core::{
    Action, ActionCoordinates, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionSideEffect,
    ActionTargetSelector, ClickMode, DragMotion, ImageSizePx, Locator, OperatorError,
    PermissionStatus, PermissionsReport, Point, Rect, TypeTrailingKey,
};
use tempfile::NamedTempFile;
use tokio::sync::oneshot;

use crate::{
    action::{
        clear_before_key_codes, click_detail, drag_warnings, parse_hotkey_keys, parse_key_code,
        point_coordinates, press_detail, range_coordinates, successful_action_outcome,
        swipe_warnings, trailing_key_code, type_side_effect, unsupported_action_error,
        velocity_from_duration,
    },
    errors::hdc_platform_error,
    normalize::{resolve_action_target, target_anchor_point, ResolvedActionTarget},
    permissions::{HarmonyPermissionSnapshot, ProbeStatus},
    HarmonyHdcConfig,
};

const SHELL_PROBE_COMMAND: &str = "echo operator >/dev/null";

pub trait HarmonyHdcShellSession {
    fn exec_checked(&mut self, command: &str) -> Result<(), OperatorError>;
    fn screenshot_probe(&mut self) -> Result<(), OperatorError>;
    fn capture_screenshot(&mut self, path: &Path) -> Result<(), OperatorError>;
    fn display_size(&mut self) -> Result<ImageSizePx, OperatorError>;
    fn focused_window_bounds(&mut self) -> Result<Option<Rect>, OperatorError>;
    fn list_apps(&mut self) -> Result<Vec<String>, OperatorError>;
    fn current_app(&mut self) -> Result<Option<CurrentApp>, OperatorError>;
    fn list_windows_with_missions(&mut self) -> Result<CorrelatedWindowList, OperatorError>;
    fn click(&mut self, point: Point, mode: ClickMode) -> Result<(), OperatorError>;
    fn input_text(&mut self, text: &str) -> Result<(), OperatorError>;
    fn press_keys(&mut self, keys: &[u32]) -> Result<(), OperatorError>;
    fn drag(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError>;
    fn swipe(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError>;
}

pub trait HarmonyHdcUiSession {
    fn check_ready(&self) -> Result<(), OperatorError>;
    fn resolve_locator(&mut self, locator: &Locator) -> Result<Option<Point>, OperatorError>;
}

pub trait HarmonyHdcSessionFactory: Send + Sync + 'static {
    fn connect_shell(
        &self,
        config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcShellSession>, OperatorError>;

    fn connect_ui(
        &self,
        config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RealHarmonyHdcSessionFactory;

impl HarmonyHdcSessionFactory for RealHarmonyHdcSessionFactory {
    fn connect_shell(
        &self,
        config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcShellSession>, OperatorError> {
        let driver = build_driver(config)?;
        Ok(Box::new(RealHarmonyHdcShellSession { driver }))
    }

    fn connect_ui(
        &self,
        config: &HarmonyHdcConfig,
    ) -> Result<Box<dyn HarmonyHdcUiSession>, OperatorError> {
        let mut builder = UiDriver::builder(config.addr().to_string())
            .connect_key(config.connect_key().to_string())
            .timeout(config.timeout())
            .remote_agent_path(config.remote_agent_path().to_string())
            .startup_delay(config.startup_delay());
        if let Some(key_dir) = config.key_dir() {
            builder = builder.key_dir(key_dir.to_path_buf());
        }
        if let Some(agent_path) = config.agent_path() {
            builder = builder.agent_path(agent_path.to_path_buf());
        }

        let ui = builder
            .connect()
            .map_err(|error| hdc_platform_error("failed to establish Harmony UI bridge", error))?;
        Ok(Box::new(RealHarmonyHdcUiSession { ui }))
    }
}

#[derive(Debug, Clone)]
pub struct HarmonyHdcWorker {
    config: HarmonyHdcConfig,
    sender: mpsc::Sender<WorkerCommand>,
}

impl HarmonyHdcWorker {
    pub fn new(config: HarmonyHdcConfig) -> Self {
        Self::new_with_session_factory(config, Arc::new(RealHarmonyHdcSessionFactory))
    }

    pub fn config(&self) -> &HarmonyHdcConfig {
        &self.config
    }

    pub async fn permissions_report(&self) -> Result<PermissionsReport, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::CollectPermissions {
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn capture_observe(
        &self,
        artifact_path: Option<PathBuf>,
        resolve_frontmost_bounds: bool,
    ) -> Result<HarmonyCaptureReport, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::CaptureObserve {
                artifact_path,
                resolve_frontmost_bounds,
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn query_apps(&self) -> Result<HarmonyAppQueryReport, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::QueryApps {
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn query_windows(&self) -> Result<CorrelatedWindowList, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::QueryWindows {
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn act(&self, request: ActionRequest) -> Result<ActionOutcome, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::Act {
                request: Box::new(request),
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) fn new_with_session_factory(
        config: HarmonyHdcConfig,
        session_factory: Arc<dyn HarmonyHdcSessionFactory>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let worker_config = config.clone();
        let _ = thread::spawn(move || worker_loop(worker_config, session_factory, receiver));

        Self { config, sender }
    }
}

enum WorkerCommand {
    CollectPermissions {
        response: oneshot::Sender<Result<PermissionsReport, OperatorError>>,
    },
    CaptureObserve {
        artifact_path: Option<PathBuf>,
        resolve_frontmost_bounds: bool,
        response: oneshot::Sender<Result<HarmonyCaptureReport, OperatorError>>,
    },
    QueryApps {
        response: oneshot::Sender<Result<HarmonyAppQueryReport, OperatorError>>,
    },
    QueryWindows {
        response: oneshot::Sender<Result<CorrelatedWindowList, OperatorError>>,
    },
    Act {
        request: Box<ActionRequest>,
        response: oneshot::Sender<Result<ActionOutcome, OperatorError>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HarmonyCaptureReport {
    pub(crate) image_size_px: ImageSizePx,
    pub(crate) focused_window_bounds: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HarmonyAppQueryReport {
    pub(crate) bundles: Vec<String>,
    pub(crate) current_app: Option<CurrentApp>,
}

struct WorkerState {
    config: HarmonyHdcConfig,
    session_factory: Arc<dyn HarmonyHdcSessionFactory>,
    shell_session: Option<Box<dyn HarmonyHdcShellSession>>,
    ui_session: Option<Box<dyn HarmonyHdcUiSession>>,
}

impl WorkerState {
    fn new(config: HarmonyHdcConfig, session_factory: Arc<dyn HarmonyHdcSessionFactory>) -> Self {
        Self {
            config,
            session_factory,
            shell_session: None,
            ui_session: None,
        }
    }

    fn permissions_report(&mut self) -> PermissionsReport {
        self.permission_snapshot().report()
    }

    fn permission_snapshot(&mut self) -> HarmonyPermissionSnapshot {
        let connect = match self.ensure_shell_session() {
            Ok(()) => ProbeStatus::granted(),
            Err(error) => {
                let message = error.to_string();
                return HarmonyPermissionSnapshot {
                    connect: ProbeStatus::denied(message.clone()),
                    shell: ProbeStatus::skipped(format!("skipped because {message}")),
                    capture: ProbeStatus::skipped(format!("skipped because {message}")),
                    ui_bridge: ProbeStatus::skipped(format!("skipped because {message}")),
                };
            }
        };

        let shell = match self.probe_shell() {
            Ok(()) => ProbeStatus::granted(),
            Err(error) => ProbeStatus::denied(error.to_string()),
        };

        let capture = if shell.status == PermissionStatus::Granted {
            match self.probe_capture() {
                Ok(()) => ProbeStatus::granted(),
                Err(error) => ProbeStatus::denied(error.to_string()),
            }
        } else {
            ProbeStatus::skipped("skipped because hdc.shell is not ready")
        };

        let ui_bridge = match self.probe_ui() {
            Ok(()) => ProbeStatus::granted(),
            Err(error) => ProbeStatus::denied(error.to_string()),
        };

        HarmonyPermissionSnapshot {
            connect,
            shell,
            capture,
            ui_bridge,
        }
    }

    fn ensure_shell_session(&mut self) -> Result<(), OperatorError> {
        if self.shell_session.is_none() {
            self.shell_session = Some(self.session_factory.connect_shell(&self.config)?);
        }
        Ok(())
    }

    fn probe_shell(&mut self) -> Result<(), OperatorError> {
        self.ensure_shell_session()?;
        let result = self
            .shell_session
            .as_mut()
            .expect("shell session should be initialized")
            .exec_checked(SHELL_PROBE_COMMAND);
        if result.is_err() {
            self.shell_session = None;
        }
        result
    }

    fn probe_capture(&mut self) -> Result<(), OperatorError> {
        self.ensure_shell_session()?;
        self.shell_session
            .as_mut()
            .expect("shell session should be initialized")
            .screenshot_probe()
    }

    fn probe_ui(&mut self) -> Result<(), OperatorError> {
        if self.ui_session.is_none() {
            self.ui_session = Some(self.session_factory.connect_ui(&self.config)?);
        }

        let result = self
            .ui_session
            .as_ref()
            .expect("ui session should be initialized")
            .check_ready();
        if result.is_err() {
            self.ui_session = None;
        }
        result
    }

    fn ensure_ui_session(&mut self) -> Result<(), OperatorError> {
        if self.ui_session.is_none() {
            self.ui_session = Some(self.session_factory.connect_ui(&self.config)?);
        }
        Ok(())
    }

    fn capture_observe(
        &mut self,
        artifact_path: Option<&Path>,
        resolve_frontmost_bounds: bool,
    ) -> Result<HarmonyCaptureReport, OperatorError> {
        self.ensure_shell_session()?;

        let result = {
            let session = self
                .shell_session
                .as_mut()
                .expect("shell session should be initialized");

            if let Some(path) = artifact_path {
                session.capture_screenshot(path)?;
            }

            let image_size_px = session.display_size()?;
            let focused_window_bounds = if resolve_frontmost_bounds {
                session.focused_window_bounds().ok().flatten()
            } else {
                None
            };

            Ok(HarmonyCaptureReport {
                image_size_px,
                focused_window_bounds,
            })
        };

        if result.is_err() {
            self.shell_session = None;
        }

        result
    }

    fn act(&mut self, request: ActionRequest) -> Result<ActionOutcome, OperatorError> {
        let ActionRequest {
            action,
            locator,
            target_selector,
            focus_policy,
            verifications: _,
        } = request;
        let started = Instant::now();
        let resolved_target = target_selector
            .as_ref()
            .map(|selector| self.resolve_target(selector))
            .transpose()?;

        let mut outcome = match action {
            Action::Click { mode } => self.click(locator, resolved_target.as_ref(), mode)?,
            Action::Type {
                text,
                clear_before,
                delay_ms: _,
                trailing_keys,
            } => self.type_text(
                locator,
                resolved_target.as_ref(),
                focus_policy,
                &text,
                clear_before,
                &trailing_keys,
            )?,
            Action::Press { key, count } => {
                self.press(resolved_target.as_ref(), focus_policy, &key, count.get())?
            }
            Action::Hotkey { keys } => {
                self.hotkey(resolved_target.as_ref(), focus_policy, &keys)?
            }
            Action::Drag { from, to, motion } => self.drag(from, to, motion)?,
            Action::Swipe {
                from,
                to,
                duration_ms,
                steps,
            } => self.swipe(from, to, duration_ms, steps)?,
            other => return Err(unsupported_action_error(&other)),
        };

        apply_target(&mut outcome, resolved_target.as_ref());
        outcome.duration_ms = started.elapsed().as_millis() as u64;
        Ok(outcome)
    }

    fn query_apps(&mut self) -> Result<HarmonyAppQueryReport, OperatorError> {
        self.ensure_shell_session()?;

        let result = {
            let session = self
                .shell_session
                .as_mut()
                .expect("shell session should be initialized");
            let bundles = session.list_apps()?;
            let current_app = session.current_app()?;

            Ok(HarmonyAppQueryReport {
                bundles,
                current_app,
            })
        };

        if result.is_err() {
            self.shell_session = None;
        }

        result
    }

    fn query_windows(&mut self) -> Result<CorrelatedWindowList, OperatorError> {
        self.ensure_shell_session()?;

        let result = self
            .shell_session
            .as_mut()
            .expect("shell session should be initialized")
            .list_windows_with_missions();

        if result.is_err() {
            self.shell_session = None;
        }

        result
    }

    fn click(
        &mut self,
        locator: Option<Locator>,
        target: Option<&ResolvedActionTarget>,
        mode: ClickMode,
    ) -> Result<ActionOutcome, OperatorError> {
        let point = if let Some(locator) = locator.as_ref() {
            self.resolve_locator_point(locator)?
        } else {
            target.map(target_anchor).transpose()?.ok_or_else(|| {
                OperatorError::Platform(
                    "harmony.hdc click requires a locator or target selector".into(),
                )
            })?
        };
        self.with_shell_session(|session| session.click(point, mode))?;

        let mut outcome = successful_action_outcome(click_detail(mode));
        outcome.coordinates = Some(point_coordinates(point));
        outcome.side_effects = vec![ActionSideEffect::Click { mode }];
        Ok(outcome)
    }

    fn type_text(
        &mut self,
        locator: Option<Locator>,
        target: Option<&ResolvedActionTarget>,
        focus_policy: ActionFocusPolicy,
        text: &str,
        clear_before: bool,
        trailing_keys: &[TypeTrailingKey],
    ) -> Result<ActionOutcome, OperatorError> {
        let focus_point = if let Some(locator) = locator.as_ref() {
            Some(self.resolve_locator_point(locator)?)
        } else if matches!(focus_policy, ActionFocusPolicy::Auto) {
            target.map(target_anchor).transpose()?
        } else {
            None
        };

        let mut side_effects = Vec::new();
        if let Some(point) = focus_point {
            self.with_shell_session(|session| session.click(point, ClickMode::Left))?;
            side_effects.push(ActionSideEffect::Click {
                mode: ClickMode::Left,
            });
        }

        if clear_before {
            let clear_keys = clear_before_key_codes();
            self.with_shell_session(|session| session.press_keys(&clear_keys))?;
            self.with_shell_session(|session| {
                session.press_keys(&[trailing_key_code(TypeTrailingKey::Delete)])
            })?;
        }

        self.with_shell_session(|session| session.input_text(text))?;
        for key in trailing_keys {
            self.with_shell_session(|session| session.press_keys(&[trailing_key_code(*key)]))?;
        }

        let mut outcome = successful_action_outcome("typed");
        outcome.coordinates = Some(ActionCoordinates {
            point: focus_point,
            from: None,
            to: None,
        });
        outcome.side_effects = side_effects;
        outcome
            .side_effects
            .push(type_side_effect(clear_before, trailing_keys));
        Ok(outcome)
    }

    fn press(
        &mut self,
        target: Option<&ResolvedActionTarget>,
        focus_policy: ActionFocusPolicy,
        key: &str,
        count: u32,
    ) -> Result<ActionOutcome, OperatorError> {
        let mut side_effects = Vec::new();
        if matches!(focus_policy, ActionFocusPolicy::Auto) {
            if let Some(point) = target.map(target_anchor).transpose()? {
                self.with_shell_session(|session| session.click(point, ClickMode::Left))?;
                side_effects.push(ActionSideEffect::Click {
                    mode: ClickMode::Left,
                });
            }
        }

        let key_code = parse_key_code(key)?;
        for _ in 0..count {
            self.with_shell_session(|session| session.press_keys(&[key_code]))?;
        }

        let mut outcome = successful_action_outcome(press_detail(key, count));
        outcome.side_effects = side_effects;
        outcome.side_effects.push(ActionSideEffect::Press {
            key: key.to_string(),
            count,
        });
        Ok(outcome)
    }

    fn hotkey(
        &mut self,
        target: Option<&ResolvedActionTarget>,
        focus_policy: ActionFocusPolicy,
        keys: &[String],
    ) -> Result<ActionOutcome, OperatorError> {
        let mut side_effects = Vec::new();
        if matches!(focus_policy, ActionFocusPolicy::Auto) {
            if let Some(point) = target.map(target_anchor).transpose()? {
                self.with_shell_session(|session| session.click(point, ClickMode::Left))?;
                side_effects.push(ActionSideEffect::Click {
                    mode: ClickMode::Left,
                });
            }
        }

        let key_codes = parse_hotkey_keys(keys)?;
        self.with_shell_session(|session| session.press_keys(&key_codes))?;

        let mut outcome = successful_action_outcome("sent hotkey");
        outcome.side_effects = side_effects;
        outcome.side_effects.push(ActionSideEffect::Hotkey {
            keys: keys.to_vec(),
        });
        Ok(outcome)
    }

    fn drag(
        &mut self,
        from: Locator,
        to: Locator,
        motion: DragMotion,
    ) -> Result<ActionOutcome, OperatorError> {
        let from_point = self.resolve_locator_point(&from)?;
        let to_point = self.resolve_locator_point(&to)?;
        let speed = velocity_from_duration(from_point, to_point, motion.duration_ms);
        self.with_shell_session(|session| session.drag(from_point, to_point, speed))?;

        let mut outcome = successful_action_outcome("dragged");
        outcome.coordinates = Some(range_coordinates(from_point, to_point));
        outcome.side_effects = vec![ActionSideEffect::Drag {
            motion: motion.clone(),
        }];
        outcome.warnings.extend(drag_warnings(&motion));
        Ok(outcome)
    }

    fn swipe(
        &mut self,
        from: Locator,
        to: Locator,
        duration_ms: Option<u64>,
        steps: Option<std::num::NonZeroU32>,
    ) -> Result<ActionOutcome, OperatorError> {
        let from_point = self.resolve_locator_point(&from)?;
        let to_point = self.resolve_locator_point(&to)?;
        let speed = velocity_from_duration(from_point, to_point, duration_ms);
        self.with_shell_session(|session| session.swipe(from_point, to_point, speed))?;

        let mut outcome = successful_action_outcome("swiped");
        outcome.coordinates = Some(range_coordinates(from_point, to_point));
        outcome.side_effects = vec![ActionSideEffect::Swipe { duration_ms, steps }];
        outcome.warnings.extend(swipe_warnings(steps));
        Ok(outcome)
    }

    fn resolve_target(
        &mut self,
        selector: &ActionTargetSelector,
    ) -> Result<ResolvedActionTarget, OperatorError> {
        let windows = self.query_windows()?;
        let current_app = self.current_app()?;
        resolve_action_target(windows, current_app, selector)
    }

    fn resolve_locator_point(&mut self, locator: &Locator) -> Result<Point, OperatorError> {
        match locator {
            Locator::Coords(point) => Ok(*point),
            Locator::Text(_) | Locator::Role { .. } => self
                .with_ui_session(|session| session.resolve_locator(locator))?
                .ok_or_else(|| {
                    OperatorError::Platform(format!(
                        "harmony.hdc could not resolve locator `{locator:?}`"
                    ))
                }),
            Locator::SnapshotElement { .. }
            | Locator::SnapshotPixelCoords { .. }
            | Locator::SnapshotCoords { .. }
            | Locator::SnapshotNormalizedCoords { .. } => Err(OperatorError::Platform(
                "harmony.hdc received an unresolved snapshot locator".into(),
            )),
        }
    }

    fn current_app(&mut self) -> Result<Option<CurrentApp>, OperatorError> {
        self.with_shell_session(|session| session.current_app())
    }

    fn with_shell_session<T>(
        &mut self,
        op: impl FnOnce(&mut dyn HarmonyHdcShellSession) -> Result<T, OperatorError>,
    ) -> Result<T, OperatorError> {
        self.ensure_shell_session()?;
        let result = {
            let session = self
                .shell_session
                .as_mut()
                .expect("shell session should be initialized");
            op(session.as_mut())
        };
        if result.is_err() {
            self.shell_session = None;
        }
        result
    }

    fn with_ui_session<T>(
        &mut self,
        op: impl FnOnce(&mut dyn HarmonyHdcUiSession) -> Result<T, OperatorError>,
    ) -> Result<T, OperatorError> {
        self.ensure_ui_session()?;
        let result = {
            let session = self
                .ui_session
                .as_mut()
                .expect("ui session should be initialized");
            op(session.as_mut())
        };
        if result.is_err() {
            self.ui_session = None;
        }
        result
    }
}

struct RealHarmonyHdcShellSession {
    driver: Driver,
}

impl HarmonyHdcShellSession for RealHarmonyHdcShellSession {
    fn exec_checked(&mut self, command: &str) -> Result<(), OperatorError> {
        let result = self
            .driver
            .shell(command)
            .map_err(|error| hdc_platform_error("failed to execute shell over hdc", error))?;
        ensure_shell_success(command, &result)
    }

    fn screenshot_probe(&mut self) -> Result<(), OperatorError> {
        let temp = NamedTempFile::new()?;
        self.driver
            .screenshot(temp.path())
            .map_err(|error| hdc_platform_error("failed to capture screenshot over hdc", error))?;
        Ok(())
    }

    fn capture_screenshot(&mut self, path: &Path) -> Result<(), OperatorError> {
        self.driver
            .screenshot(path)
            .map_err(|error| hdc_platform_error("failed to capture screenshot over hdc", error))?;
        Ok(())
    }

    fn display_size(&mut self) -> Result<ImageSizePx, OperatorError> {
        let point = self
            .driver
            .display_size()
            .map_err(|error| hdc_platform_error("failed to read harmony display size", error))?;
        image_size_from_point(point.x, point.y)
    }

    fn focused_window_bounds(&mut self) -> Result<Option<Rect>, OperatorError> {
        let windows = self.driver.list_windows().map_err(|error| {
            hdc_platform_error(
                "failed to read Harmony focused window bounds for frontmost observe",
                error,
            )
        })?;
        let Some(focused_window_id) = windows.focused_window_id else {
            return Ok(None);
        };
        let Some(window) = windows
            .windows
            .into_iter()
            .find(|window| window.window_id == focused_window_id)
        else {
            return Ok(None);
        };

        Ok(Some(Rect {
            x: f64::from(window.rect.x),
            y: f64::from(window.rect.y),
            width: f64::from(window.rect.width),
            height: f64::from(window.rect.height),
        }))
    }

    fn list_apps(&mut self) -> Result<Vec<String>, OperatorError> {
        self.driver
            .list_apps(true)
            .map_err(|error| hdc_platform_error("failed to list Harmony apps", error))
    }

    fn current_app(&mut self) -> Result<Option<CurrentApp>, OperatorError> {
        self.driver
            .current_app()
            .map_err(|error| hdc_platform_error("failed to read Harmony foreground app", error))
    }

    fn list_windows_with_missions(&mut self) -> Result<CorrelatedWindowList, OperatorError> {
        self.driver
            .list_windows_with_missions()
            .map_err(|error| hdc_platform_error("failed to list Harmony windows", error))
    }

    fn click(&mut self, point: Point, mode: ClickMode) -> Result<(), OperatorError> {
        let (x, y) = screen_point(point)?;
        match mode {
            ClickMode::Left => self
                .driver
                .click(x, y)
                .map_err(|error| hdc_platform_error("failed to click over hdc", error)),
            ClickMode::Right => self
                .driver
                .right_click(x, y)
                .map_err(|error| hdc_platform_error("failed to right click over hdc", error)),
            ClickMode::Double => self
                .driver
                .double_click(x, y)
                .map_err(|error| hdc_platform_error("failed to double click over hdc", error)),
            ClickMode::Middle => Err(OperatorError::Platform(
                "harmony.hdc does not support middle click in the first phase".into(),
            )),
        }
    }

    fn input_text(&mut self, text: &str) -> Result<(), OperatorError> {
        self.driver
            .input_text(text)
            .map_err(|error| hdc_platform_error("failed to type text over hdc", error))
    }

    fn press_keys(&mut self, keys: &[u32]) -> Result<(), OperatorError> {
        self.driver
            .press_keys(keys.iter().copied())
            .map_err(|error| hdc_platform_error("failed to press keys over hdc", error))
    }

    fn drag(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError> {
        let (from_x, from_y) = screen_point(from)?;
        let (to_x, to_y) = screen_point(to)?;
        self.driver
            .drag(from_x, from_y, to_x, to_y, speed)
            .map_err(|error| hdc_platform_error("failed to drag over hdc", error))
    }

    fn swipe(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError> {
        let (from_x, from_y) = screen_point(from)?;
        let (to_x, to_y) = screen_point(to)?;
        self.driver
            .swipe(from_x, from_y, to_x, to_y, speed)
            .map_err(|error| hdc_platform_error("failed to swipe over hdc", error))
    }
}

struct RealHarmonyHdcUiSession {
    ui: UiDriver,
}

impl HarmonyHdcUiSession for RealHarmonyHdcUiSession {
    fn check_ready(&self) -> Result<(), OperatorError> {
        self.ui.display_size().map(|_| ()).map_err(|error| {
            hdc_platform_error("failed to verify harmony ui bridge readiness", error)
        })
    }

    fn resolve_locator(&mut self, locator: &Locator) -> Result<Option<Point>, OperatorError> {
        match locator {
            Locator::Coords(point) => Ok(Some(*point)),
            Locator::Text(text) => self
                .ui
                .query()
                .text(text.clone())
                .find_component()
                .and_then(|component| {
                    component
                        .map(|component| {
                            component.center().map(|point| Point {
                                x: f64::from(point.x),
                                y: f64::from(point.y),
                            })
                        })
                        .transpose()
                })
                .map_err(|error| {
                    hdc_platform_error("failed to resolve Harmony text locator", error)
                }),
            Locator::Role { role, index } => self
                .ui
                .query()
                .kind(role.clone())
                .index(*index)
                .find_component()
                .and_then(|component| {
                    component
                        .map(|component| {
                            component.center().map(|point| Point {
                                x: f64::from(point.x),
                                y: f64::from(point.y),
                            })
                        })
                        .transpose()
                })
                .map_err(|error| {
                    hdc_platform_error("failed to resolve Harmony role locator", error)
                }),
            Locator::SnapshotElement { .. }
            | Locator::SnapshotPixelCoords { .. }
            | Locator::SnapshotCoords { .. }
            | Locator::SnapshotNormalizedCoords { .. } => Err(OperatorError::Platform(
                "harmony.hdc cannot resolve snapshot locators through the ui bridge".into(),
            )),
        }
    }
}

fn worker_loop(
    config: HarmonyHdcConfig,
    session_factory: Arc<dyn HarmonyHdcSessionFactory>,
    receiver: mpsc::Receiver<WorkerCommand>,
) {
    let mut state = WorkerState::new(config, session_factory);
    while let Ok(command) = receiver.recv() {
        match command {
            WorkerCommand::CollectPermissions { response } => {
                let _ = response.send(Ok(state.permissions_report()));
            }
            WorkerCommand::CaptureObserve {
                artifact_path,
                resolve_frontmost_bounds,
                response,
            } => {
                let _ = response.send(
                    state.capture_observe(artifact_path.as_deref(), resolve_frontmost_bounds),
                );
            }
            WorkerCommand::QueryApps { response } => {
                let _ = response.send(state.query_apps());
            }
            WorkerCommand::QueryWindows { response } => {
                let _ = response.send(state.query_windows());
            }
            WorkerCommand::Act { request, response } => {
                let _ = response.send(state.act(*request));
            }
        }
    }
}

fn build_driver(config: &HarmonyHdcConfig) -> Result<Driver, OperatorError> {
    let mut builder = Driver::builder(config.addr().to_string())
        .connect_key(config.connect_key().to_string())
        .timeout(config.timeout());
    if let Some(key_dir) = config.key_dir() {
        builder = builder.key_dir(key_dir.to_path_buf());
    }

    builder
        .connect()
        .map_err(|error| hdc_platform_error("failed to establish hdc session", error))
}

fn ensure_shell_success(command: &str, result: &ShellResult) -> Result<(), OperatorError> {
    if result.failed() {
        let message = result
            .messages
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(OperatorError::Platform(format!(
            "shell command `{command}` failed over hdc: {message}"
        )));
    }
    Ok(())
}

fn worker_stopped_error() -> OperatorError {
    OperatorError::Platform("harmony.hdc worker has stopped".into())
}

fn image_size_from_point(width: i32, height: i32) -> Result<ImageSizePx, OperatorError> {
    if width <= 0 || height <= 0 {
        return Err(OperatorError::Platform(format!(
            "harmony.hdc reported invalid display size: {width}x{height}"
        )));
    }

    Ok(ImageSizePx {
        width: width as u32,
        height: height as u32,
    })
}

fn apply_target(outcome: &mut ActionOutcome, target: Option<&ResolvedActionTarget>) {
    if let Some(target) = target {
        outcome.target_app = target.app.clone();
        outcome.target_window = target.window.clone();
    }
}

fn target_anchor(target: &ResolvedActionTarget) -> Result<Point, OperatorError> {
    target_anchor_point(target).ok_or_else(|| {
        OperatorError::Platform(
            "harmony.hdc cannot derive a target anchor because the resolved window has no bounds"
                .into(),
        )
    })
}

fn screen_point(point: Point) -> Result<(i32, i32), OperatorError> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(OperatorError::Platform(
            "harmony.hdc received a non-finite coordinate".into(),
        ));
    }
    if point.x < f64::from(i32::MIN)
        || point.x > f64::from(i32::MAX)
        || point.y < f64::from(i32::MIN)
        || point.y > f64::from(i32::MAX)
    {
        return Err(OperatorError::Platform(
            "harmony.hdc received an out-of-range coordinate".into(),
        ));
    }

    Ok((point.x.round() as i32, point.y.round() as i32))
}
