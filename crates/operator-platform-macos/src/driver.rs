use async_trait::async_trait;
use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, CapabilitySet, ExecContext, HealthStatus,
    ObserveRequest, ObserveResult, OperatorError, PermissionStatus, QueryRequest, QueryResult,
};

use crate::{AppService, PermissionReader, SystemAppService, SystemPermissionReader};

pub struct MacosDriver<A = SystemAppService, P = SystemPermissionReader> {
    app_service: A,
    permission_reader: P,
}

impl MacosDriver<SystemAppService, SystemPermissionReader> {
    pub fn system() -> Self {
        Self::default()
    }
}

impl<A, P> MacosDriver<A, P> {
    pub fn new(app_service: A, permission_reader: P) -> Self {
        Self {
            app_service,
            permission_reader,
        }
    }

    pub fn app_service(&self) -> &A {
        &self.app_service
    }

    pub fn permission_reader(&self) -> &P {
        &self.permission_reader
    }
}

impl Default for MacosDriver<SystemAppService, SystemPermissionReader> {
    fn default() -> Self {
        Self::new(SystemAppService, SystemPermissionReader)
    }
}

fn macos_capabilities() -> CapabilitySet {
    CapabilitySet::new([
        Capability::AppLifecycle,
        Capability::WindowManagement,
        Capability::Permissions,
    ])
}

#[async_trait]
impl<A, P> operator_core::PlatformDriver for MacosDriver<A, P>
where
    A: AppService,
    P: PermissionReader,
{
    fn platform_id(&self) -> &'static str {
        "macos"
    }

    fn capabilities(&self) -> CapabilitySet {
        macos_capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus, OperatorError> {
        let permissions = self.permission_reader.current_permissions()?;
        let healthy = permissions.accessibility == PermissionStatus::Granted;
        let message = if healthy {
            None
        } else {
            Some("Accessibility permission is required for macOS automation.".into())
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
        _ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError> {
        let capability = if req.include_screenshot {
            Capability::Capture
        } else {
            Capability::InspectTree
        };

        Err(OperatorError::CapabilityNotSupported(capability))
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
