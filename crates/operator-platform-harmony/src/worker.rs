use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
};

use hmdriver_rs::{Driver, ShellResult, UiDriver};
use operator_core::{ImageSizePx, OperatorError, PermissionStatus, PermissionsReport};
use tempfile::NamedTempFile;
use tokio::sync::oneshot;

use crate::{
    errors::hdc_platform_error,
    permissions::{HarmonyPermissionSnapshot, ProbeStatus},
    HarmonyHdcConfig,
};

const SHELL_PROBE_COMMAND: &str = "echo operator >/dev/null";

pub trait HarmonyHdcShellSession {
    fn exec_checked(&mut self, command: &str) -> Result<(), OperatorError>;
    fn screenshot_probe(&mut self) -> Result<(), OperatorError>;
    fn capture_screenshot(&mut self, path: &Path) -> Result<(), OperatorError>;
    fn display_size(&mut self) -> Result<ImageSizePx, OperatorError>;
}

pub trait HarmonyHdcUiSession {
    fn check_ready(&self) -> Result<(), OperatorError>;
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
    ) -> Result<HarmonyCaptureReport, OperatorError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(WorkerCommand::CaptureObserve {
                artifact_path,
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
        response: oneshot::Sender<Result<HarmonyCaptureReport, OperatorError>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HarmonyCaptureReport {
    pub(crate) image_size_px: ImageSizePx,
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

    fn capture_observe(
        &mut self,
        artifact_path: Option<&Path>,
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

            session
                .display_size()
                .map(|image_size_px| HarmonyCaptureReport { image_size_px })
        };

        if result.is_err() {
            self.shell_session = None;
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
                response,
            } => {
                let _ = response.send(state.capture_observe(artifact_path.as_deref()));
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
