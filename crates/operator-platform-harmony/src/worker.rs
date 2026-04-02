use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

use hmdriver_rs::{
    AppLabelInfo, Bounds, CorrelatedWindowList, CurrentApp, Driver, ShellResult, UiComponentInfo,
    UiDriver,
};
use operator_core::{
    Action, ActionCoordinates, ActionFocusPolicy, ActionOutcome, ActionRequest, ActionSideEffect,
    ActionTargetSelector, AppInfo, Capability, ClickMode, DragMotion, FocusInfo, ImageSizePx,
    Locator, OperatorError, PermissionStatus, PermissionsReport, Point, Rect, TargetId,
    TypeTrailingKey, WindowInfo,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::NamedTempFile;
use tokio::sync::oneshot;

use crate::{
    action::{
        action_name, clear_before_key_codes, click_detail, drag_warnings, parse_hotkey_keys,
        parse_key_code, point_coordinates, press_detail, range_coordinates,
        successful_action_outcome, swipe_warnings, trailing_key_code, type_side_effect,
        unsupported_action_error, velocity_from_duration,
    },
    errors::hdc_platform_error,
    inspect::{build_inspect_result, filter_inspect_result_to_region, InspectResult},
    normalize::{
        normalize_windows, resolve_action_target, target_anchor_point, InstalledHarmonyApp,
        ResolvedActionTarget,
    },
    permissions::{HarmonyPermissionSnapshot, ProbeStatus},
    HarmonyHdcConfig,
};

const SHELL_PROBE_COMMAND: &str = "echo operator >/dev/null";
const APP_CATALOG_CACHE_VERSION: u32 = 1;
const SMART_DOCK_WINDOW_ID: &str = "11";

pub trait HarmonyHdcShellSession: Send {
    fn exec_checked(&mut self, command: &str) -> Result<(), OperatorError>;
    fn screenshot_probe(&mut self) -> Result<(), OperatorError>;
    fn capture_screenshot(&mut self, path: &Path) -> Result<(), OperatorError>;
    fn display_size(&mut self) -> Result<ImageSizePx, OperatorError>;
    fn focused_window_bounds(&mut self) -> Result<Option<Rect>, OperatorError>;
    fn list_apps(&mut self) -> Result<Vec<String>, OperatorError>;
    fn list_app_labels(&mut self) -> Result<Vec<AppLabelInfo>, OperatorError>;
    fn filter_desktop_bundles(&mut self, bundles: &[String]) -> Result<Vec<String>, OperatorError>;
    fn current_app(&mut self) -> Result<Option<CurrentApp>, OperatorError>;
    fn list_windows_with_missions(&mut self) -> Result<CorrelatedWindowList, OperatorError>;
    fn dump_hierarchy(&mut self) -> Result<Value, OperatorError> {
        Err(OperatorError::CapabilityNotSupported(
            Capability::InspectTree,
        ))
    }
    fn move_cursor(&mut self, _point: Point) -> Result<(), OperatorError> {
        Err(OperatorError::CapabilityNotSupported(
            Capability::PointerInput,
        ))
    }
    fn dock_app_icon_point_by_bundle(
        &mut self,
        _bundle_id: &str,
    ) -> Result<Option<Point>, OperatorError> {
        Err(OperatorError::CapabilityNotSupported(
            Capability::AppLifecycle,
        ))
    }
    fn click(&mut self, point: Point, mode: ClickMode) -> Result<(), OperatorError>;
    fn input_text(&mut self, text: &str) -> Result<(), OperatorError>;
    fn press_keys(&mut self, keys: &[u32]) -> Result<(), OperatorError>;
    fn start_app(&mut self, bundle: &str, ability: Option<&str>) -> Result<(), OperatorError>;
    fn stop_app(&mut self, bundle: &str) -> Result<(), OperatorError>;
    fn drag(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError>;
    fn swipe(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError>;
}

pub trait HarmonyHdcUiSession {
    fn check_ready(&self) -> Result<(), OperatorError>;
    fn resolve_locator(&mut self, locator: &Locator) -> Result<Option<Point>, OperatorError>;
    fn active_window(&mut self) -> Result<Option<HarmonyActiveWindow>, OperatorError> {
        Err(OperatorError::CapabilityNotSupported(
            Capability::WindowQuery,
        ))
    }
    fn focused_component(&mut self) -> Result<Option<UiComponentInfo>, OperatorError> {
        Err(OperatorError::CapabilityNotSupported(
            Capability::InspectTree,
        ))
    }
    fn dock_app_icon_point(&mut self, _labels: &[String]) -> Result<Option<Point>, OperatorError> {
        Err(OperatorError::CapabilityNotSupported(
            Capability::AppLifecycle,
        ))
    }
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
    pub fn new(target_id: TargetId, config: HarmonyHdcConfig, cache_root: PathBuf) -> Self {
        Self::new_with_session_factory(
            target_id,
            config,
            Arc::new(RealHarmonyHdcSessionFactory),
            cache_root,
        )
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

    pub(crate) async fn query_apps_with_refresh(
        &self,
        refresh: bool,
    ) -> Result<HarmonyAppQueryReport, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::QueryApps {
                refresh,
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn cached_apps(&self) -> Result<Option<HarmonyAppQueryReport>, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::LoadCachedApps {
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn query_app_labels_map(
        &self,
    ) -> Result<BTreeMap<String, String>, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::QueryAppLabels {
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

    pub(crate) async fn query_focus(&self) -> Result<Option<FocusInfo>, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::QueryFocus {
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) async fn inspect_tree(
        &self,
        region: Option<Rect>,
    ) -> Result<InspectResult, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::InspectTree {
                region,
                response: response_tx,
            })
            .map_err(|_| worker_stopped_error())?;

        response_rx.await.map_err(|_| worker_stopped_error())?
    }

    pub(crate) fn new_with_session_factory(
        target_id: TargetId,
        config: HarmonyHdcConfig,
        session_factory: Arc<dyn HarmonyHdcSessionFactory>,
        cache_root: PathBuf,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let worker_config = config.clone();
        let _ = thread::spawn(move || {
            worker_loop(
                target_id,
                worker_config,
                session_factory,
                cache_root,
                receiver,
            )
        });

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
        refresh: bool,
        response: oneshot::Sender<Result<HarmonyAppQueryReport, OperatorError>>,
    },
    LoadCachedApps {
        response: oneshot::Sender<Result<Option<HarmonyAppQueryReport>, OperatorError>>,
    },
    QueryAppLabels {
        response: oneshot::Sender<Result<BTreeMap<String, String>, OperatorError>>,
    },
    QueryWindows {
        response: oneshot::Sender<Result<CorrelatedWindowList, OperatorError>>,
    },
    InspectTree {
        region: Option<Rect>,
        response: oneshot::Sender<Result<InspectResult, OperatorError>>,
    },
    QueryFocus {
        response: oneshot::Sender<Result<Option<FocusInfo>, OperatorError>>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HarmonyAppQueryReport {
    pub(crate) installed_apps: Vec<InstalledHarmonyApp>,
    pub(crate) labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HarmonyAppCatalogCache {
    version: u32,
    report: HarmonyAppQueryReport,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HarmonyActiveWindow {
    pub bundle_id: String,
    pub title: Option<String>,
    pub bounds: Option<Rect>,
    pub is_focused: bool,
}

struct WorkerState {
    target_id: TargetId,
    config: HarmonyHdcConfig,
    cache_root: PathBuf,
    session_factory: Arc<dyn HarmonyHdcSessionFactory>,
    shell_session: Option<Box<dyn HarmonyHdcShellSession>>,
    ui_session: Option<Box<dyn HarmonyHdcUiSession>>,
    app_catalog_cache: Option<HarmonyAppQueryReport>,
}

impl WorkerState {
    fn new(
        target_id: TargetId,
        config: HarmonyHdcConfig,
        session_factory: Arc<dyn HarmonyHdcSessionFactory>,
        cache_root: PathBuf,
    ) -> Self {
        Self {
            target_id,
            config,
            cache_root,
            session_factory,
            shell_session: None,
            ui_session: None,
            app_catalog_cache: None,
        }
    }

    fn permissions_report(&mut self) -> PermissionsReport {
        self.permission_snapshot().report()
    }

    fn permission_snapshot(&mut self) -> HarmonyPermissionSnapshot {
        let probe_budget = self.permissions_probe_budget();
        let started = Instant::now();

        let connect = match self.ensure_shell_session_for_permissions(
            self.permission_probe_timeout(started, probe_budget, Duration::from_secs(3)),
        ) {
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

        let shell = match self.probe_shell_for_permissions(self.permission_probe_timeout(
            started,
            probe_budget,
            Duration::from_secs(1),
        )) {
            Ok(()) => ProbeStatus::granted(),
            Err(error) => ProbeStatus::denied(error.to_string()),
        };

        let capture = if shell.status == PermissionStatus::Granted {
            match self.probe_capture_for_permissions(self.permission_probe_timeout(
                started,
                probe_budget,
                Duration::from_secs(1),
            )) {
                Ok(()) => ProbeStatus::granted(),
                Err(error) => ProbeStatus::denied(error.to_string()),
            }
        } else {
            ProbeStatus::skipped("skipped because hdc.shell is not ready")
        };

        let ui_bridge = match self.probe_ui_for_permissions(self.permission_probe_timeout(
            started,
            probe_budget,
            probe_budget,
        )) {
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

    fn permissions_probe_budget(&self) -> Duration {
        self.config.timeout().min(Duration::from_secs(9))
    }

    fn permission_probe_timeout(
        &self,
        started: Instant,
        budget: Duration,
        cap: Duration,
    ) -> Duration {
        let elapsed = started.elapsed();
        let remaining = budget.saturating_sub(elapsed);
        if remaining.is_zero() {
            return Duration::from_millis(1);
        }
        remaining.min(cap)
    }

    fn ensure_shell_session_for_permissions(
        &mut self,
        timeout: Duration,
    ) -> Result<(), OperatorError> {
        if self.shell_session.is_none() {
            let factory = Arc::clone(&self.session_factory);
            let config = self.config.clone();
            self.shell_session = Some(run_probe_with_timeout(
                timeout,
                "hdc shell connect",
                move || factory.connect_shell(&config),
            )?);
        }
        Ok(())
    }

    fn ensure_shell_session(&mut self) -> Result<(), OperatorError> {
        if self.shell_session.is_none() {
            self.shell_session = Some(self.session_factory.connect_shell(&self.config)?);
        }
        Ok(())
    }

    fn probe_shell_for_permissions(&mut self, timeout: Duration) -> Result<(), OperatorError> {
        self.with_shell_session_timeout(timeout, "hdc shell probe", |session| {
            session.exec_checked(SHELL_PROBE_COMMAND)
        })
    }

    fn probe_capture_for_permissions(&mut self, timeout: Duration) -> Result<(), OperatorError> {
        self.with_shell_session_timeout(timeout, "hdc capture probe", |session| {
            session.screenshot_probe()
        })
    }

    fn probe_ui_for_permissions(&mut self, timeout: Duration) -> Result<(), OperatorError> {
        if let Some(session) = self.ui_session.as_ref() {
            match session.check_ready() {
                Ok(()) => return Ok(()),
                Err(_) => self.ui_session = None,
            }
        }
        let factory = Arc::clone(&self.session_factory);
        let config = self.config.clone();
        run_probe_with_timeout(timeout, "harmony ui bridge probe", move || {
            let session = factory.connect_ui(&config)?;
            session.check_ready()
        })
    }

    fn ensure_ui_session(&mut self) -> Result<(), OperatorError> {
        if self.ui_session.is_none() {
            self.ui_session = Some(self.session_factory.connect_ui(&self.config)?);
        }
        Ok(())
    }

    fn with_shell_session_timeout<T>(
        &mut self,
        timeout: Duration,
        label: &'static str,
        op: impl FnOnce(&mut dyn HarmonyHdcShellSession) -> Result<T, OperatorError> + Send + 'static,
    ) -> Result<T, OperatorError>
    where
        T: Send + 'static,
    {
        self.ensure_shell_session_for_permissions(timeout)?;

        let session = self
            .shell_session
            .take()
            .expect("shell session should be initialized");
        let (tx, rx) = mpsc::channel();
        let _ = thread::spawn(move || {
            let mut session = session;
            let result = op(session.as_mut());
            let _ = tx.send((session, result));
        });

        match rx.recv_timeout(timeout) {
            Ok((session, result)) => {
                if result.is_ok() {
                    self.shell_session = Some(session);
                }
                result
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(probe_timeout_error(label, timeout)),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(OperatorError::Platform(format!("{label} worker stopped")))
            }
        }
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

    fn inspect_tree(&mut self, region: Option<Rect>) -> Result<InspectResult, OperatorError> {
        self.ensure_shell_session()?;

        let hierarchy = {
            let session = self
                .shell_session
                .as_mut()
                .expect("shell session should be initialized");
            session.dump_hierarchy()
        };

        let hierarchy = match hierarchy {
            Ok(hierarchy) => hierarchy,
            Err(error) => {
                self.shell_session = None;
                return Err(error);
            }
        };

        let result = build_inspect_result(hierarchy)?;
        Ok(match region {
            Some(region) => filter_inspect_result_to_region(result, region),
            None => result,
        })
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
        let resolved_target = if matches!(&action, Action::UnhideApp) {
            None
        } else {
            target_selector
                .as_ref()
                .map(|selector| self.resolve_target(selector))
                .transpose()?
        };

        let mut outcome = match action {
            Action::LaunchApp { bundle_id_or_name } => self.launch_app(&bundle_id_or_name)?,
            Action::Click { mode } => self.click(locator, resolved_target.as_ref(), mode)?,
            Action::Move => self.move_cursor(locator, resolved_target.as_ref())?,
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
            Action::Scroll { delta_x, delta_y } => {
                self.scroll(locator, resolved_target.as_ref(), delta_x, delta_y)?
            }
            Action::SwitchApp => {
                let target = required_target_selector("switch-app", resolved_target.as_ref())?;
                self.switch_app(target)?
            }
            Action::QuitApp => {
                let target = required_target_selector("quit-app", resolved_target.as_ref())?;
                self.quit_app(target)?
            }
            Action::RelaunchApp => {
                let target = required_target_selector("relaunch-app", resolved_target.as_ref())?;
                self.relaunch_app(target)?
            }
            Action::HideApp => {
                let target = required_target_selector("hide-app", resolved_target.as_ref())?;
                if !hide_target_matches_frontmost_window(target) {
                    return Err(OperatorError::Platform(
                        "harmony.hdc hide-app requires the target app to be frontmost and visible"
                            .into(),
                    ));
                }
                self.toggle_app_visibility(target, "hid app", ActionSideEffect::HideApp)?
            }
            Action::UnhideApp => {
                let target = self.resolve_unhide_target(target_selector.as_ref())?;
                self.toggle_app_visibility(&target, "unhid app", ActionSideEffect::UnhideApp)?
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

    fn query_focus(&mut self) -> Result<Option<FocusInfo>, OperatorError> {
        let labels = self.query_app_labels()?;
        let active_window = self.best_effort_active_window(true);
        let current_app = self.frontmost_app(active_window.as_ref())?;
        let focused_window = focused_window_from_windows(self.query_windows()?, &labels);
        let bundle_id = active_window
            .as_ref()
            .map(|window| window.bundle_id.clone())
            .or_else(|| current_app.as_ref().map(|app| app.bundle_name.clone()));
        let app_name = resolved_focus_app_name(
            bundle_id.as_deref(),
            &labels,
            focused_window.as_ref(),
            active_window.as_ref(),
        );
        if let Some(component) = self.best_effort_focused_component() {
            return Ok(Some(focus_info_from_component(
                component, bundle_id, app_name,
            )));
        }

        if let Some(window) = active_window {
            return Ok(Some(FocusInfo {
                role: "Window".into(),
                label: window.title.clone().or_else(|| app_name.clone()),
                bounds: window.bounds,
                bundle_id: Some(window.bundle_id),
                app_name,
            }));
        }

        if let Some(window) = focused_window {
            return Ok(Some(FocusInfo {
                role: "Window".into(),
                label: window.title.clone().or_else(|| app_name.clone()),
                bounds: window.bounds,
                bundle_id,
                app_name,
            }));
        }

        if let Some(bundle_id) = bundle_id {
            let label = app_name.clone();
            return Ok(Some(FocusInfo {
                role: "Application".into(),
                label,
                bounds: None,
                bundle_id: Some(bundle_id),
                app_name,
            }));
        }

        Ok(None)
    }

    fn launch_app(&mut self, bundle_id_or_name: &str) -> Result<ActionOutcome, OperatorError> {
        let bundle = self.resolve_launch_bundle(bundle_id_or_name)?;
        self.with_shell_session(|session| session.start_app(&bundle, None))?;

        let mut outcome = successful_action_outcome(format!("launched {bundle}"));
        outcome.side_effects = vec![ActionSideEffect::LaunchApp];
        Ok(outcome)
    }

    fn query_apps(&mut self, refresh: bool) -> Result<HarmonyAppQueryReport, OperatorError> {
        if !refresh {
            if let Some(report) = &self.app_catalog_cache {
                return Ok(report.clone());
            }
            if let Some(report) = self.load_app_catalog_cache() {
                self.app_catalog_cache = Some(report.clone());
                return Ok(report);
            }
        }

        let report = self.rebuild_app_catalog()?;
        self.app_catalog_cache = Some(report.clone());
        let _ = self.persist_app_catalog_cache(&report);
        Ok(report)
    }

    fn cached_apps(&mut self) -> Result<Option<HarmonyAppQueryReport>, OperatorError> {
        if let Some(report) = &self.app_catalog_cache {
            return Ok(Some(report.clone()));
        }

        let report = self.load_app_catalog_cache();
        if let Some(report) = &report {
            self.app_catalog_cache = Some(report.clone());
        }
        Ok(report)
    }

    fn rebuild_app_catalog(&mut self) -> Result<HarmonyAppQueryReport, OperatorError> {
        self.ensure_shell_session()?;
        let labels = self.query_app_labels()?;
        let bundles = {
            let session = self
                .shell_session
                .as_mut()
                .expect("shell session should be initialized");
            session.list_apps()?
        };

        let mut installed_apps = BTreeMap::new();
        for bundle in bundles {
            let bundle = bundle.trim().to_string();
            let Some(label) = labels.get(&bundle) else {
                continue;
            };
            let label = label.trim();
            if !looks_like_gui_catalog_entry(&bundle, label) {
                continue;
            }
            installed_apps
                .entry(bundle.clone())
                .or_insert_with(|| InstalledHarmonyApp {
                    bundle_id: bundle,
                    name: label.to_string(),
                });
        }

        let desktop_bundles = self.with_shell_session(|session| {
            let bundles = installed_apps.keys().cloned().collect::<Vec<_>>();
            session.filter_desktop_bundles(&bundles)
        })?;
        let desktop_bundles = desktop_bundles.into_iter().collect::<BTreeSet<_>>();

        Ok(HarmonyAppQueryReport {
            installed_apps: installed_apps
                .into_values()
                .filter(|app| desktop_bundles.contains(&app.bundle_id))
                .collect(),
            labels,
        })
    }

    fn load_app_catalog_cache(&self) -> Option<HarmonyAppQueryReport> {
        let path = self.app_catalog_cache_path();
        let bytes = fs::read(path).ok()?;
        let cache = serde_json::from_slice::<HarmonyAppCatalogCache>(&bytes).ok()?;
        (cache.version == APP_CATALOG_CACHE_VERSION).then_some(cache.report)
    }

    fn persist_app_catalog_cache(
        &self,
        report: &HarmonyAppQueryReport,
    ) -> Result<(), OperatorError> {
        fs::create_dir_all(self.app_catalog_cache_dir())?;
        let payload = HarmonyAppCatalogCache {
            version: APP_CATALOG_CACHE_VERSION,
            report: report.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&payload)?;
        fs::write(self.app_catalog_cache_path(), bytes)?;
        Ok(())
    }

    fn app_catalog_cache_dir(&self) -> PathBuf {
        self.cache_root.join("cache").join("harmony-apps")
    }

    fn app_catalog_cache_path(&self) -> PathBuf {
        self.app_catalog_cache_dir()
            .join(format!("{}.json", sanitize_target_id(&self.target_id)))
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

    fn query_app_labels(&mut self) -> Result<BTreeMap<String, String>, OperatorError> {
        if let Some(report) = &self.app_catalog_cache {
            return Ok(report.labels.clone());
        }

        self.ensure_shell_session()?;
        let result = {
            let session = self
                .shell_session
                .as_mut()
                .expect("shell session should be initialized");
            session.list_app_labels()
        };

        match result {
            Ok(labels) => Ok(labels
                .into_iter()
                .filter_map(|item| {
                    let bundle = item.bundle_name.trim();
                    let label = item.label.trim();
                    if bundle.is_empty() || label.is_empty() {
                        return None;
                    }
                    Some((bundle.to_string(), label.to_string()))
                })
                .collect()),
            Err(error) => {
                self.shell_session = None;
                Err(error)
            }
        }
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

    fn move_cursor(
        &mut self,
        locator: Option<Locator>,
        target: Option<&ResolvedActionTarget>,
    ) -> Result<ActionOutcome, OperatorError> {
        let point = if let Some(locator) = locator.as_ref() {
            self.resolve_locator_point(locator)?
        } else {
            target.map(target_anchor).transpose()?.ok_or_else(|| {
                OperatorError::Platform(
                    "harmony.hdc move requires a locator or target selector".into(),
                )
            })?
        };
        self.with_shell_session(|session| session.move_cursor(point))?;

        let mut outcome = successful_action_outcome("moved");
        outcome.coordinates = Some(point_coordinates(point));
        outcome.side_effects = vec![ActionSideEffect::MoveCursor];
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

    fn switch_app(
        &mut self,
        target: &ResolvedActionTarget,
    ) -> Result<ActionOutcome, OperatorError> {
        let bundle = lifecycle_target_bundle(target, "switch-app")?;
        self.with_shell_session(|session| session.start_app(&bundle, None))?;

        let mut outcome = successful_action_outcome("switched app");
        outcome.side_effects = vec![ActionSideEffect::SwitchApp];
        apply_target(&mut outcome, Some(target));
        Ok(outcome)
    }

    fn quit_app(&mut self, target: &ResolvedActionTarget) -> Result<ActionOutcome, OperatorError> {
        let bundle = lifecycle_target_bundle(target, "quit-app")?;
        self.with_shell_session(|session| session.stop_app(&bundle))?;

        let mut outcome = successful_action_outcome("quit app");
        outcome.side_effects = vec![ActionSideEffect::QuitApp];
        apply_target(&mut outcome, Some(target));
        Ok(outcome)
    }

    fn relaunch_app(
        &mut self,
        target: &ResolvedActionTarget,
    ) -> Result<ActionOutcome, OperatorError> {
        let bundle = lifecycle_target_bundle(target, "relaunch-app")?;
        self.with_shell_session(|session| session.stop_app(&bundle))?;
        self.with_shell_session(|session| session.start_app(&bundle, None))?;

        let mut outcome = successful_action_outcome("relaunched app");
        outcome.side_effects = vec![ActionSideEffect::RelaunchApp];
        apply_target(&mut outcome, Some(target));
        Ok(outcome)
    }

    fn toggle_app_visibility(
        &mut self,
        target: &ResolvedActionTarget,
        detail: &str,
        side_effect: ActionSideEffect,
    ) -> Result<ActionOutcome, OperatorError> {
        let candidates = self.dock_icon_labels(target)?;
        let point = match self.dock_icon_point_by_bundle(target)? {
            Some(point) => Some(point),
            None => self
                .with_ui_session(|session| session.dock_app_icon_point(&candidates))
                .unwrap_or_default(),
        }
        .ok_or_else(|| {
            OperatorError::Platform(format!(
                "harmony.hdc could not find a dock icon for {}",
                candidates.join(" / ")
            ))
        })?;
        self.with_shell_session(|session| session.click(point, ClickMode::Left))?;

        let mut outcome = successful_action_outcome(detail);
        outcome.coordinates = Some(point_coordinates(point));
        outcome.side_effects = vec![
            ActionSideEffect::Click {
                mode: ClickMode::Left,
            },
            side_effect,
        ];
        outcome
            .warnings
            .push("harmony.hdc approximates app visibility by clicking the dock icon".into());
        apply_target(&mut outcome, Some(target));
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

    fn scroll(
        &mut self,
        locator: Option<Locator>,
        target: Option<&ResolvedActionTarget>,
        delta_x: f64,
        delta_y: f64,
    ) -> Result<ActionOutcome, OperatorError> {
        let point = if let Some(locator) = locator.as_ref() {
            self.resolve_locator_point(locator)?
        } else if let Some(target) = target {
            target_anchor(target)?
        } else {
            display_center(self.with_shell_session(|session| session.display_size())?)
        };

        let mut outcome = successful_action_outcome("scrolled");
        outcome.coordinates = Some(point_coordinates(point));
        outcome.side_effects = vec![ActionSideEffect::Scroll { delta_x, delta_y }];
        outcome
            .warnings
            .push("harmony.hdc approximates scroll with swipe input".into());

        if delta_x.abs() <= f64::EPSILON && delta_y.abs() <= f64::EPSILON {
            return Ok(outcome);
        }

        let display = self.with_shell_session(|session| session.display_size())?;
        let (from, to) = scroll_swipe_segment(point, display, delta_x, delta_y);
        self.with_shell_session(|session| session.swipe(from, to, Some(2_000)))?;
        outcome.coordinates = Some(range_coordinates(from, to));
        Ok(outcome)
    }

    fn resolve_target(
        &mut self,
        selector: &ActionTargetSelector,
    ) -> Result<ResolvedActionTarget, OperatorError> {
        let windows = self.query_windows()?;
        let active_window = self.best_effort_active_window(false);
        let current_app = self.frontmost_app(active_window.as_ref())?;
        let labels = match selector {
            ActionTargetSelector::App(_) => self.query_app_labels()?,
            _ => BTreeMap::new(),
        };
        resolve_action_target(windows, current_app, &labels, selector)
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

    fn best_effort_active_window(&mut self, connect: bool) -> Option<HarmonyActiveWindow> {
        if !connect && self.ui_session.is_none() {
            return None;
        }
        self.with_ui_session(|session| session.active_window())
            .unwrap_or_default()
    }

    fn best_effort_focused_component(&mut self) -> Option<UiComponentInfo> {
        self.with_ui_session(|session| session.focused_component())
            .unwrap_or_default()
    }

    fn frontmost_app(
        &mut self,
        active_window: Option<&HarmonyActiveWindow>,
    ) -> Result<Option<CurrentApp>, OperatorError> {
        if let Some(window) = active_window {
            let bundle = window.bundle_id.trim();
            if !bundle.is_empty() {
                return Ok(Some(CurrentApp {
                    bundle_name: bundle.to_string(),
                    ability_name: String::new(),
                }));
            }
        }
        self.current_app()
    }

    fn resolve_launch_bundle(&mut self, bundle_id_or_name: &str) -> Result<String, OperatorError> {
        let bundle_id_or_name = bundle_id_or_name.trim();
        if bundle_id_or_name.is_empty() {
            return Err(OperatorError::Platform(
                "harmony.hdc launch-app requires a non-empty bundle id or app name".into(),
            ));
        }

        let requested = normalize_match_text(bundle_id_or_name);
        let installed = self.with_shell_session(|session| session.list_apps())?;
        if let Some(bundle) = installed
            .into_iter()
            .find(|bundle| normalize_match_text(bundle) == requested)
        {
            return Ok(bundle);
        }

        let active_window = self.best_effort_active_window(false);
        let current_app = self.frontmost_app(active_window.as_ref())?;
        if let Some(bundle) = current_app
            .as_ref()
            .filter(|app| normalize_match_text(&app.bundle_name) == requested)
            .map(|app| app.bundle_name.clone())
        {
            return Ok(bundle);
        }

        let labels = self.query_app_labels()?;
        let windows = self.query_windows()?;
        let selector = ActionTargetSelector::App(bundle_id_or_name.to_string());
        if let Ok(target) = resolve_action_target(windows, current_app, &labels, &selector) {
            return lifecycle_target_bundle(
                &target,
                action_name(&Action::LaunchApp {
                    bundle_id_or_name: bundle_id_or_name.to_string(),
                }),
            );
        }

        if let Some(bundle) = self.resolve_installed_app_bundle_by_name(&requested)? {
            return Ok(bundle);
        }

        Err(OperatorError::Platform(format!(
            "harmony.hdc could not resolve `{bundle_id_or_name}` to an installed bundle id or running app"
        )))
    }

    fn resolve_installed_app_bundle_by_name(
        &mut self,
        requested: &str,
    ) -> Result<Option<String>, OperatorError> {
        if let Some(report) = self.cached_apps()? {
            if let Some(bundle) =
                find_installed_app_bundle_by_name(&report.installed_apps, requested)
            {
                return Ok(Some(bundle));
            }
        }

        let report = self.rebuild_app_catalog()?;
        self.app_catalog_cache = Some(report.clone());
        let _ = self.persist_app_catalog_cache(&report);
        Ok(find_installed_app_bundle_by_name(
            &report.installed_apps,
            requested,
        ))
    }

    fn resolve_unhide_target(
        &mut self,
        selector: Option<&ActionTargetSelector>,
    ) -> Result<ResolvedActionTarget, OperatorError> {
        let selector = selector.ok_or_else(|| {
            OperatorError::Platform("harmony.hdc unhide-app requires a target selector".into())
        })?;

        Ok(match selector {
            ActionTargetSelector::App(bundle_id_or_name) => {
                self.resolve_installed_or_running_app_target(bundle_id_or_name)?
            }
            _ => self.resolve_target(selector)?,
        })
    }

    fn resolve_installed_or_running_app_target(
        &mut self,
        bundle_id_or_name: &str,
    ) -> Result<ResolvedActionTarget, OperatorError> {
        if let Ok(target) =
            self.resolve_target(&ActionTargetSelector::App(bundle_id_or_name.into()))
        {
            return Ok(target);
        }

        let bundle = self.resolve_launch_bundle(bundle_id_or_name)?;
        let labels = self.query_app_labels()?;
        let name = labels
            .get(&bundle)
            .cloned()
            .unwrap_or_else(|| bundle.clone());

        Ok(ResolvedActionTarget {
            app: Some(AppInfo {
                bundle_id: Some(bundle),
                name,
                pid: None,
                is_running: false,
            }),
            window: None,
        })
    }

    fn dock_icon_labels(
        &mut self,
        target: &ResolvedActionTarget,
    ) -> Result<Vec<String>, OperatorError> {
        let labels = self.query_app_labels()?;
        let mut candidates = Vec::new();
        if let Some(app) = target.app.as_ref() {
            push_unique_label(&mut candidates, &app.name);
            if let Some(bundle_id) = app.bundle_id.as_deref() {
                push_unique_label(&mut candidates, bundle_id);
                if let Some(label) = labels.get(bundle_id) {
                    push_unique_label(&mut candidates, label);
                }
            }
        }
        if let Some(window) = target.window.as_ref() {
            if let Some(app_name) = window.app_name.as_deref() {
                push_unique_label(&mut candidates, app_name);
            }
        }

        if candidates.is_empty() {
            return Err(OperatorError::Platform(
                "harmony.hdc app visibility fallback requires a resolved app label".into(),
            ));
        }

        Ok(candidates)
    }

    fn dock_icon_point_by_bundle(
        &mut self,
        target: &ResolvedActionTarget,
    ) -> Result<Option<Point>, OperatorError> {
        let bundle_id = target
            .app
            .as_ref()
            .and_then(|app| app.bundle_id.as_deref())
            .and_then(non_empty_str);
        let Some(bundle_id) = bundle_id else {
            return Ok(None);
        };
        self.with_shell_session(|session| session.dock_app_icon_point_by_bundle(bundle_id))
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

    fn list_app_labels(&mut self) -> Result<Vec<AppLabelInfo>, OperatorError> {
        self.driver
            .list_app_labels()
            .map_err(|error| hdc_platform_error("failed to list Harmony app labels", error))
    }

    fn current_app(&mut self) -> Result<Option<CurrentApp>, OperatorError> {
        self.driver
            .current_app()
            .map_err(|error| hdc_platform_error("failed to read Harmony foreground app", error))
    }

    fn filter_desktop_bundles(&mut self, bundles: &[String]) -> Result<Vec<String>, OperatorError> {
        self.driver
            .filter_desktop_bundles(bundles)
            .map_err(|error| hdc_platform_error("failed to filter Harmony desktop apps", error))
    }

    fn list_windows_with_missions(&mut self) -> Result<CorrelatedWindowList, OperatorError> {
        self.driver
            .list_windows_with_missions()
            .map_err(|error| hdc_platform_error("failed to list Harmony windows", error))
    }

    fn dump_hierarchy(&mut self) -> Result<Value, OperatorError> {
        self.driver
            .dump_hierarchy()
            .map_err(|error| hdc_platform_error("failed to dump Harmony layout hierarchy", error))
    }

    fn move_cursor(&mut self, point: Point) -> Result<(), OperatorError> {
        let (x, y) = screen_point(point)?;
        self.driver
            .move_cursor(x, y)
            .map_err(|error| hdc_platform_error("failed to move cursor over hdc", error))
    }

    fn dock_app_icon_point_by_bundle(
        &mut self,
        bundle_id: &str,
    ) -> Result<Option<Point>, OperatorError> {
        let hierarchy = self.driver.dump_hierarchy().map_err(|error| {
            hdc_platform_error(
                "failed to dump Harmony layout hierarchy for dock lookup",
                error,
            )
        })?;
        Ok(find_smart_dock_icon_point(&hierarchy, bundle_id))
    }

    fn click(&mut self, point: Point, mode: ClickMode) -> Result<(), OperatorError> {
        let (x, y) = screen_point(point)?;
        match mode {
            ClickMode::Left => self
                .driver
                .click_abs(x, y)
                .map_err(|error| hdc_platform_error("failed to click over hdc", error)),
            ClickMode::Right => self
                .driver
                .right_click_abs(x, y)
                .map_err(|error| hdc_platform_error("failed to right click over hdc", error)),
            ClickMode::Double => self
                .driver
                .double_click_abs(x, y)
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

    fn start_app(&mut self, bundle: &str, ability: Option<&str>) -> Result<(), OperatorError> {
        self.driver
            .start_app(bundle, ability)
            .map_err(|error| hdc_platform_error("failed to start Harmony app over hdc", error))
    }

    fn stop_app(&mut self, bundle: &str) -> Result<(), OperatorError> {
        self.driver
            .stop_app(bundle)
            .map_err(|error| hdc_platform_error("failed to stop Harmony app over hdc", error))
    }

    fn drag(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError> {
        let (from_x, from_y) = screen_point(from)?;
        let (to_x, to_y) = screen_point(to)?;
        self.driver
            .drag_abs(from_x, from_y, to_x, to_y, speed)
            .map_err(|error| hdc_platform_error("failed to drag over hdc", error))
    }

    fn swipe(&mut self, from: Point, to: Point, speed: Option<u32>) -> Result<(), OperatorError> {
        let (from_x, from_y) = screen_point(from)?;
        let (to_x, to_y) = screen_point(to)?;
        self.driver
            .swipe_abs(from_x, from_y, to_x, to_y, speed)
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

    fn active_window(&mut self) -> Result<Option<HarmonyActiveWindow>, OperatorError> {
        self.ui
            .find_active_window()
            .and_then(|window| {
                window
                    .map(|window| {
                        let title = non_empty_owned(window.title()?);
                        Ok(HarmonyActiveWindow {
                            bundle_id: window.bundle_name()?,
                            title,
                            bounds: rect_from_ui_bounds(window.bounds()?),
                            is_focused: window.is_focused()?,
                        })
                    })
                    .transpose()
            })
            .map_err(|error| hdc_platform_error("failed to resolve Harmony active window", error))
    }

    fn focused_component(&mut self) -> Result<Option<UiComponentInfo>, OperatorError> {
        self.ui
            .query()
            .focused(true)
            .find_component()
            .and_then(|component| component.map(|component| component.info()).transpose())
            .map_err(|error| {
                hdc_platform_error("failed to resolve Harmony focused component", error)
            })
    }

    fn dock_app_icon_point(&mut self, labels: &[String]) -> Result<Option<Point>, OperatorError> {
        let display = self.ui.display_size().map_err(|error| {
            hdc_platform_error("failed to read Harmony display size for dock lookup", error)
        })?;
        let display_height = f64::from(display.y);
        let dock_threshold = display_height * 0.75;
        let mut best: Option<(Point, f64)> = None;
        for label in labels
            .iter()
            .map(|label| label.trim())
            .filter(|label| !label.is_empty())
        {
            collect_dock_candidates(&self.ui, label, dock_threshold, &mut best).map_err(
                |error| hdc_platform_error("failed to resolve Harmony dock icon", error),
            )?;
        }

        Ok(best.map(|(point, _)| point))
    }
}

fn looks_like_gui_catalog_entry(bundle: &str, label: &str) -> bool {
    let bundle = bundle.trim();
    let label = label.trim();
    if bundle.is_empty() || label.is_empty() {
        return false;
    }

    if label.eq_ignore_ascii_case("label")
        || label.eq_ignore_ascii_case(bundle)
        || bundle
            .rsplit('.')
            .next()
            .is_some_and(|segment| label.eq_ignore_ascii_case(segment))
        || label.contains('_')
    {
        return false;
    }

    let bundle_lower = bundle.to_ascii_lowercase();
    if [
        ".data",
        ".dataservice",
        ".dialog",
        ".widget",
        ".resources",
        ".service",
        ".extension",
        ".ext",
        "data",
        "dialog",
        "widget",
        "resource",
        "service",
        "core",
        "systemres",
        "sceneboard",
        "spooler",
        "foundation",
        "autofill",
        "restores",
    ]
    .iter()
    .any(|needle| bundle_lower.contains(needle))
    {
        return false;
    }

    let label_lower = label.to_ascii_lowercase();
    if [
        "service",
        "dialog",
        "widget",
        "storage",
        "credentialmgr",
        "mgr",
        "ext",
        "fwk",
        "data",
        "core",
        "choice",
    ]
    .iter()
    .any(|needle| label_lower.contains(needle))
    {
        return false;
    }

    !["服务", "存储", "控件", "资源"]
        .iter()
        .any(|needle| label.contains(needle))
}

fn worker_loop(
    target_id: TargetId,
    config: HarmonyHdcConfig,
    session_factory: Arc<dyn HarmonyHdcSessionFactory>,
    cache_root: PathBuf,
    receiver: mpsc::Receiver<WorkerCommand>,
) {
    let mut state = WorkerState::new(target_id, config, session_factory, cache_root);
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
            WorkerCommand::QueryApps { refresh, response } => {
                let _ = response.send(state.query_apps(refresh));
            }
            WorkerCommand::QueryAppLabels { response } => {
                let _ = response.send(state.query_app_labels());
            }
            WorkerCommand::LoadCachedApps { response } => {
                let _ = response.send(state.cached_apps());
            }
            WorkerCommand::QueryWindows { response } => {
                let _ = response.send(state.query_windows());
            }
            WorkerCommand::InspectTree { region, response } => {
                let _ = response.send(state.inspect_tree(region));
            }
            WorkerCommand::QueryFocus { response } => {
                let _ = response.send(state.query_focus());
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

fn run_probe_with_timeout<T>(
    timeout: Duration,
    label: &'static str,
    op: impl FnOnce() -> Result<T, OperatorError> + Send + 'static,
) -> Result<T, OperatorError>
where
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let _ = thread::spawn(move || {
        let _ = tx.send(op());
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(probe_timeout_error(label, timeout)),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(OperatorError::Platform(format!("{label} worker stopped")))
        }
    }
}

fn probe_timeout_error(label: &str, timeout: Duration) -> OperatorError {
    OperatorError::Platform(format!("{label} timed out after {}ms", timeout.as_millis()))
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

fn focus_info_from_component(
    component: UiComponentInfo,
    bundle_id: Option<String>,
    app_name: Option<String>,
) -> FocusInfo {
    let role = non_empty_str(&component.kind)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Unknown".into());
    FocusInfo {
        role,
        label: component_label(&component),
        bounds: rect_from_ui_bounds(component.bounds),
        bundle_id,
        app_name,
    }
}

fn component_label(component: &UiComponentInfo) -> Option<String> {
    non_empty_owned(component.description.clone())
        .or_else(|| non_empty_owned(component.text.clone()))
}

fn rect_from_ui_bounds(bounds: Bounds) -> Option<Rect> {
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(Rect {
        x: f64::from(bounds.left),
        y: f64::from(bounds.top),
        width: f64::from(width),
        height: f64::from(height),
    })
}

fn focused_window_from_windows(
    windows: CorrelatedWindowList,
    labels: &BTreeMap<String, String>,
) -> Option<WindowInfo> {
    normalize_windows(windows, labels, None)
        .into_iter()
        .find(|window| window.is_focused)
}

fn resolved_focus_app_name(
    bundle_id: Option<&str>,
    labels: &BTreeMap<String, String>,
    focused_window: Option<&WindowInfo>,
    active_window: Option<&HarmonyActiveWindow>,
) -> Option<String> {
    bundle_id
        .and_then(|bundle| labels.get(bundle).cloned())
        .or_else(|| focused_window.and_then(|window| window.app_name.clone()))
        .or_else(|| active_window.and_then(|window| window.title.clone()))
        .or_else(|| bundle_id.map(ToOwned::to_owned))
}

fn hide_target_matches_frontmost_window(target: &ResolvedActionTarget) -> bool {
    target
        .window
        .as_ref()
        .is_some_and(|window| window.is_focused)
}

fn display_center(size: ImageSizePx) -> Point {
    Point {
        x: f64::from(size.width) / 2.0,
        y: f64::from(size.height) / 2.0,
    }
}

fn scroll_swipe_segment(
    anchor: Point,
    display: ImageSizePx,
    delta_x: f64,
    delta_y: f64,
) -> (Point, Point) {
    let width = f64::from(display.width);
    let height = f64::from(display.height);
    let margin = width.min(height).mul_add(0.04, 0.0).clamp(24.0, 64.0);
    let swipe_dx = scroll_axis_swipe_distance(delta_x, width, false);
    let swipe_dy = scroll_axis_swipe_distance(delta_y, height, true);
    let (from_x, to_x) = bounded_axis_segment(anchor.x, swipe_dx, margin, width - margin);
    let (from_y, to_y) = bounded_axis_segment(anchor.y, swipe_dy, margin, height - margin);
    (
        Point {
            x: from_x,
            y: from_y,
        },
        Point { x: to_x, y: to_y },
    )
}

fn scroll_axis_swipe_distance(delta: f64, axis_extent: f64, vertical: bool) -> f64 {
    if delta.abs() <= f64::EPSILON {
        return 0.0;
    }

    let max_distance = (axis_extent * 0.35).max(24.0);
    let distance = ((delta.abs() / 120.0).max(0.25) * axis_extent * 0.12).clamp(24.0, max_distance);
    let sign = if vertical {
        delta.signum()
    } else {
        -delta.signum()
    };
    sign * distance
}

fn bounded_axis_segment(anchor: f64, travel: f64, min: f64, max: f64) -> (f64, f64) {
    let half = travel / 2.0;
    let mut start = anchor - half;
    let mut end = anchor + half;
    let low = start.min(end);
    let mut high = start.max(end);
    if low < min {
        let shift = min - low;
        start += shift;
        end += shift;
        high += shift;
    }
    if high > max {
        let shift = high - max;
        start -= shift;
        end -= shift;
    }

    (start.clamp(min, max), end.clamp(min, max))
}

fn push_unique_label(labels: &mut Vec<String>, value: &str) {
    let Some(value) = non_empty_str(value) else {
        return;
    };
    if labels
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
    {
        return;
    }
    labels.push(value.to_string());
}

#[derive(Debug, Default, Deserialize)]
struct DockLayoutNode {
    #[serde(default)]
    attributes: DockLayoutAttributes,
    #[serde(default)]
    children: Vec<DockLayoutNode>,
}

#[allow(non_snake_case)]
#[derive(Debug, Default, Deserialize)]
struct DockLayoutAttributes {
    #[serde(default)]
    bounds: Option<String>,
    #[serde(default)]
    clickable: Option<String>,
    #[serde(default)]
    hostWindowId: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    key: Option<String>,
}

fn find_smart_dock_icon_point(hierarchy: &Value, bundle_id: &str) -> Option<Point> {
    let root = serde_json::from_value::<DockLayoutNode>(hierarchy.clone()).ok()?;
    let mut best: Option<(Point, u8, f64)> = None;
    collect_smart_dock_icon_candidates(&root, bundle_id, &mut best);
    best.map(|(point, _, _)| point)
}

fn collect_smart_dock_icon_candidates(
    node: &DockLayoutNode,
    bundle_id: &str,
    best: &mut Option<(Point, u8, f64)>,
) {
    if let Some((point, score, area)) = smart_dock_icon_candidate(node, bundle_id) {
        let replace = match best {
            Some((_, best_score, best_area)) => {
                score > *best_score || (score == *best_score && area < *best_area)
            }
            None => true,
        };
        if replace {
            *best = Some((point, score, area));
        }
    }

    for child in &node.children {
        collect_smart_dock_icon_candidates(child, bundle_id, best);
    }
}

fn smart_dock_icon_candidate(node: &DockLayoutNode, bundle_id: &str) -> Option<(Point, u8, f64)> {
    let score = smart_dock_match_score(&node.attributes, bundle_id)?;
    let bounds = node
        .attributes
        .bounds
        .as_deref()
        .and_then(parse_harmony_bounds)?;
    let point = Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };
    Some((point, score, bounds.width * bounds.height))
}

fn smart_dock_match_score(attributes: &DockLayoutAttributes, bundle_id: &str) -> Option<u8> {
    if attributes.hostWindowId.as_deref() != Some(SMART_DOCK_WINDOW_ID)
        || attributes.clickable.as_deref() != Some("true")
    {
        return None;
    }

    let icon_container = format!("SmartDock_AppIcon_Container_{bundle_id}");
    let icon_view = format!("AppIconCommonView_{bundle_id}");
    let resident_item = format!("ResidentLayoutForPC_AppItem_{bundle_id}");
    let mut score = 0_u8;

    for candidate in [attributes.id.as_deref(), attributes.key.as_deref()]
        .into_iter()
        .flatten()
    {
        if !candidate.contains(bundle_id) {
            continue;
        }
        let candidate_score = if candidate.contains(&icon_container) {
            4
        } else if candidate.contains(&icon_view) {
            3
        } else if candidate.contains(&resident_item) {
            2
        } else {
            1
        };
        score = score.max(candidate_score);
    }

    (score > 0).then_some(score)
}

fn parse_harmony_bounds(value: &str) -> Option<Rect> {
    let value = value.trim();
    let (top_left, bottom_right) = value
        .strip_prefix('[')?
        .strip_suffix(']')?
        .split_once("][")?;
    let (left, top) = top_left.split_once(',')?;
    let (right, bottom) = bottom_right.split_once(',')?;
    let left = left.trim().parse::<f64>().ok()?;
    let top = top.trim().parse::<f64>().ok()?;
    let right = right.trim().parse::<f64>().ok()?;
    let bottom = bottom.trim().parse::<f64>().ok()?;
    let width = right - left;
    let height = bottom - top;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(Rect {
        x: left,
        y: top,
        width,
        height,
    })
}

fn non_empty_owned(value: String) -> Option<String> {
    non_empty_str(&value).map(ToOwned::to_owned)
}

fn non_empty_str(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn collect_dock_candidates(
    ui: &UiDriver,
    label: &str,
    dock_threshold: f64,
    best: &mut Option<(Point, f64)>,
) -> hmdriver_rs::Result<()> {
    for component in ui.query().text(label).all()? {
        consider_dock_candidate(component.info()?, dock_threshold, best);
    }
    for component in ui.query().description(label).all()? {
        consider_dock_candidate(component.info()?, dock_threshold, best);
    }
    Ok(())
}

fn consider_dock_candidate(
    component: UiComponentInfo,
    dock_threshold: f64,
    best: &mut Option<(Point, f64)>,
) {
    let center_y = f64::from(component.center.y);
    if center_y < dock_threshold {
        return;
    }

    let point = Point {
        x: f64::from(component.center.x),
        y: center_y,
    };
    match best {
        Some((_, best_y)) if *best_y >= center_y => {}
        _ => *best = Some((point, center_y)),
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

fn required_target_selector<'a>(
    action_name: &str,
    target: Option<&'a ResolvedActionTarget>,
) -> Result<&'a ResolvedActionTarget, OperatorError> {
    target.ok_or_else(|| {
        OperatorError::Platform(format!(
            "harmony.hdc {action_name} requires a target selector"
        ))
    })
}

fn lifecycle_target_bundle(
    target: &ResolvedActionTarget,
    action_name: &str,
) -> Result<String, OperatorError> {
    target
        .app
        .as_ref()
        .and_then(|app| app.bundle_id.as_deref())
        .map(str::trim)
        .filter(|bundle| !bundle.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            OperatorError::Platform(format!(
                "harmony.hdc {action_name} requires a resolved target with a bundle id"
            ))
        })
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn find_installed_app_bundle_by_name(
    installed_apps: &[InstalledHarmonyApp],
    requested: &str,
) -> Option<String> {
    installed_apps
        .iter()
        .find(|app| normalize_match_text(&app.name) == requested)
        .map(|app| app.bundle_id.clone())
}

fn sanitize_target_id(target_id: &TargetId) -> String {
    target_id
        .0
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use operator_core::Point;
    use serde_json::json;

    use super::{find_smart_dock_icon_point, looks_like_gui_catalog_entry};

    #[test]
    fn gui_catalog_filter_keeps_user_facing_apps() {
        assert!(looks_like_gui_catalog_entry(
            "com.huawei.hmos.notepad",
            "备忘录"
        ));
        assert!(looks_like_gui_catalog_entry(
            "com.huawei.hmos.browser",
            "浏览器"
        ));
    }

    #[test]
    fn gui_catalog_filter_rejects_internal_data_and_service_entries() {
        assert!(!looks_like_gui_catalog_entry(
            "com.ohos.medialibrary.medialibrarydata",
            "MeidaLibraryExt"
        ));
        assert!(!looks_like_gui_catalog_entry(
            "com.huawei.hmos.meetimeservice",
            "畅连通信"
        ));
        assert!(!looks_like_gui_catalog_entry(
            "com.ohos.locationdialog",
            "蓝牙"
        ));
        assert!(!looks_like_gui_catalog_entry(
            "ohos.global.systemres",
            "系统"
        ));
    }

    #[test]
    fn shell_dock_lookup_prefers_icon_bounds_from_dump_layout() {
        let hierarchy = json!({
            "attributes": { "bounds": "[0,0][2160,2160]" },
            "children": [{
                "attributes": {
                    "hostWindowId": "11",
                    "clickable": "true",
                    "id": "ResidentLayoutForPC_AppItem_com.demo.notes_0__",
                    "bounds": "[1802,1955][1905,2080]"
                },
                "children": [{
                    "attributes": {
                        "hostWindowId": "11",
                        "clickable": "true",
                        "id": "SmartDock_AppIcon_Container_com.demo.notes_MainAbility_0",
                        "bounds": "[1815,1981][1891,2057]"
                    }
                }]
            }]
        });

        assert_eq!(
            find_smart_dock_icon_point(&hierarchy, "com.demo.notes"),
            Some(Point {
                x: 1853.0,
                y: 2019.0,
            })
        );
    }
}
