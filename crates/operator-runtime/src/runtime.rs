use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use operator_core::{
    Action, ActionOutcome, ActionRequest, Capability, ExecContext, Locator, ObserveRequest,
    ObserveResult, OperatorError, PlatformDriver, QueryRequest, QueryResult, TargetDescriptor,
    TargetId,
};
use tokio::time;

use crate::{
    AuditEvent, AuditEventKind, EventSink, RuntimeConfig, SessionStore, SnapshotStore,
    TargetResolver,
};

pub struct RuntimeCore {
    pub(crate) resolver: TargetResolver,
    pub(crate) drivers: HashMap<String, Arc<dyn PlatformDriver>>,
    pub(crate) snapshots: Arc<dyn SnapshotStore>,
    pub(crate) sessions: Arc<dyn SessionStore>,
    pub(crate) event_sink: Arc<dyn EventSink>,
    pub(crate) config: RuntimeConfig,
}

impl RuntimeCore {
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn snapshots(&self) -> Arc<dyn SnapshotStore> {
        Arc::clone(&self.snapshots)
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
        let driver = self
            .drivers
            .get(&descriptor.platform)
            .cloned()
            .ok_or_else(|| OperatorError::TargetNotFound(target.to_string()))?;

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
        self.emit_invoked("act", &req, &ctx).await?;

        let started = Instant::now();
        let timeout_ms = self.timeout_ms(&ctx);
        let result = time::timeout(Duration::from_millis(timeout_ms), driver.act(req, &ctx)).await;

        match result {
            Ok(Ok(result)) => {
                self.emit_completed("act", started, true, &ctx).await?;
                Ok(result)
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
            QueryRequest::ListApps => Some(Capability::AppLifecycle),
            QueryRequest::ListWindows { .. } => Some(Capability::WindowManagement),
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
            Action::Click { .. } | Action::Scroll { .. } | Action::Drag { .. } => {
                Capability::PointerInput
            }
            Action::Type { .. } | Action::Hotkey { .. } => Capability::KeyboardInput,
            Action::LaunchApp { .. } => Capability::AppLifecycle,
            Action::FocusWindow { .. } => Capability::WindowManagement,
        };

        self.require_capability(driver, capability, "act", ctx)
            .await
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
        if let Action::Drag {
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
        } = &req.action
        {
            if from_snapshot != to_snapshot {
                return Err(OperatorError::Platform(
                    "drag: from/to must reference the same snapshot".into(),
                ));
            }
        }

        Ok(())
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

#[derive(Clone)]
pub struct Runtime {
    pub(crate) core: Arc<RuntimeCore>,
}

impl Runtime {
    pub fn core(&self) -> Arc<RuntimeCore> {
        Arc::clone(&self.core)
    }
}
