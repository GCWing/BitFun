use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bitfun_agent_runtime::sdk::{AgentRuntime, PermissionRequestEvent};
use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::product_runtime::CoreLocalWorkspaceSnapshot;
use bitfun_core::service::remote_ssh::SSHConnectionManager;
use bitfun_core::service::token_usage::TokenUsageService;
use bitfun_core::service::workspace::WorkspaceService;
use bitfun_runtime_ports::{LocalWorkspaceSnapshotPort, WardenModelJudgementPort};
use tokio::sync::RwLock;

mod acp_client_port;
mod acp_session_lifecycle;
mod session_application;
mod session_host_effects;
mod warden_model_judgement_port;

use session_host_effects::ProductionDesktopSessionHostEffects;

pub(crate) use acp_client_port::DesktopAcpClientPort;
pub(crate) use acp_session_lifecycle::AcpSessionLifecycleSubscriber;
pub(crate) use warden_model_judgement_port::DesktopWardenModelJudgementPort;

pub(crate) use session_application::{
    DesktopSessionApplication, DesktopSessionApplicationError, DesktopSessionScopeRequest,
    UiSessionMetadataField,
};

/// Desktop-owned access to the Agent Runtime SDK interaction facade.
///
/// Core remains the sole owner of the coordinator, scheduler, sessions, tool
/// pipeline, and Agentic event queue. This context exposes only the interaction
/// ports used by current Tauri commands; it does not claim that the complete
/// Desktop delivery profile or its product services have been assembled.
pub struct DesktopRuntimeContext {
    session_application: DesktopSessionApplication,
    local_workspace_snapshot: Arc<dyn LocalWorkspaceSnapshotPort>,
    /// Model-backed Warden judgement provider, assembled here and injected
    /// into the scheduler/tool-pipeline audit loop in [`Self::build`]
    /// (batch-2 warden rework). The field is intentionally held as the
    /// desktop assembly point.
    #[allow(dead_code)]
    warden_model_judgement: Arc<dyn WardenModelJudgementPort>,
    permission_events_started: AtomicBool,
}

impl DesktopRuntimeContext {
    pub(crate) fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
        workspace_service: Arc<WorkspaceService>,
        ssh_manager: Arc<RwLock<Option<SSHConnectionManager>>>,
        acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>,
        ai_client_factory: Arc<AIClientFactory>,
    ) -> Result<Self, String> {
        let host_effects = Arc::new(ProductionDesktopSessionHostEffects::new(acp_client_service));
        // Desktop-side Warden model judgement provider. Batch 2 wires this
        // port into the scheduler/tool-pipeline audit loop (the consumer);
        // the field stays as the desktop assembly point.
        let warden_model_judgement: Arc<dyn WardenModelJudgementPort> =
            Arc::new(DesktopWardenModelJudgementPort::new(ai_client_factory));
        // Batch-2 injection: the scheduler forwards the port into the tool
        // pipeline so Audit-Poke decisions go through the model provider
        // (mechanical rule ladder as fallback). Must happen before
        // `scheduler` is moved into the session application below.
        scheduler.set_warden_model_judgement(warden_model_judgement.clone());
        let session_application = DesktopSessionApplication::build(
            coordinator,
            scheduler,
            token_usage_service,
            workspace_service,
            ssh_manager,
            host_effects,
        )?;
        let local_workspace_snapshot = CoreLocalWorkspaceSnapshot::build();

        Ok(Self {
            session_application,
            local_workspace_snapshot,
            warden_model_judgement,
            permission_events_started: AtomicBool::new(false),
        })
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        self.session_application.agent_runtime()
    }

    pub(crate) fn session_application(&self) -> &DesktopSessionApplication {
        &self.session_application
    }

    pub(crate) fn local_workspace_snapshot(&self) -> &dyn LocalWorkspaceSnapshotPort {
        self.local_workspace_snapshot.as_ref()
    }

    /// Warden model judgement port held as the desktop assembly point (the
    /// active consumer is the tool pipeline via `scheduler.set_warden_model_judgement`).
    #[allow(dead_code)]
    pub(crate) fn warden_model_judgement(&self) -> Arc<dyn WardenModelJudgementPort> {
        self.warden_model_judgement.clone()
    }

    pub(crate) fn start_permission_event_forwarding(
        &self,
        app: tauri::AppHandle,
    ) -> Result<(), bitfun_agent_runtime::sdk::RuntimeError> {
        if self.permission_events_started.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let mut receiver = match self.agent_runtime().subscribe_permission_requests() {
            Ok(receiver) => receiver,
            Err(error) => {
                self.permission_events_started
                    .store(false, Ordering::Release);
                return Err(error);
            }
        };
        let runtime = self.agent_runtime().clone();
        tauri::async_runtime::spawn(async move {
            use tauri::Emitter;

            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        let fanout = crate::api::peer_host_invoke::track_permission_event(&event);
                        if fanout {
                            if let Ok(payload) = serde_json::to_value(&event) {
                                crate::api::remote_connect_api::maybe_fanout_peer_ui_event(
                                    "permission://event",
                                    payload,
                                );
                            }
                        }
                        let _ = app.emit("permission://event", event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if let Ok(requests) = runtime.pending_permission_requests() {
                            for request in requests {
                                let event = PermissionRequestEvent::Asked { request };
                                let fanout =
                                    crate::api::peer_host_invoke::track_permission_event(&event);
                                if fanout {
                                    if let Ok(payload) = serde_json::to_value(&event) {
                                        crate::api::remote_connect_api::maybe_fanout_peer_ui_event(
                                            "permission://event",
                                            payload,
                                        );
                                    }
                                }
                                let _ = app.emit("permission://event", event);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        let request_ids =
                            crate::api::peer_host_invoke::take_tracked_permission_requests();
                        if let Err(error) =
                            crate::api::peer_host_invoke::fail_closed_permission_requests(
                                request_ids,
                                "Peer permission event stream closed",
                            )
                            .await
                        {
                            log::warn!(
                                "Peer permission requests were not fully cancelled: {error}"
                            );
                        }
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn desktop_runtime_wiring_reuses_existing_core_owners() {
        let runtime_source = include_str!("mod.rs");
        let coordinator_constructor = ["ConversationCoordinator", "::new"].concat();
        let scheduler_constructor = ["DialogScheduler", "::new"].concat();
        assert!(!runtime_source.contains(&coordinator_constructor));
        assert!(!runtime_source.contains(&scheduler_constructor));

        let app_source = include_str!("../lib.rs");
        assert!(app_source.contains("DesktopRuntimeContext::build("));
        assert!(app_source.contains(".manage(desktop_runtime)"));

        assert!(runtime_source.contains("DesktopSessionApplication::build("));
        assert!(runtime_source.contains("CoreLocalWorkspaceSnapshot::build()"));

        let session_commands = include_str!("../api/session_api.rs");
        assert_eq!(
            session_commands.matches("PersistenceManager::new").count(),
            0,
            "Tauri Session commands must delegate persistence ownership to the application boundary"
        );
        let session_application = include_str!("session_application.rs");
        assert!(session_application.contains("save_persisted_dialog_turn"));
        assert!(session_application.contains("export_persisted_session_transcript"));

        let snapshot_commands = include_str!("../api/snapshot_service.rs");
        assert_eq!(
            snapshot_commands
                .matches(".local_workspace_snapshot()")
                .count(),
            1,
            "only workspace rollback uses the mutation port; reads use Core's bounded snapshot view"
        );
        assert!(snapshot_commands.contains("begin_snapshot_history_read"));
        assert!(snapshot_commands.contains("open_snapshot_manager_for_view"));
        assert!(snapshot_commands.contains("ensure_local_snapshot_mutation_path"));

        let rollback_source = &snapshot_commands[snapshot_commands
            .find("pub async fn rollback_to_turn")
            .expect("rollback command must exist")..];
        let remote_guard = rollback_source
            .find("ensure_complete_rollback_supported")
            .expect("remote rollback guard must remain host-owned");
        let maintenance = rollback_source
            .find("begin_snapshot_history_mutation")
            .expect("scheduler maintenance must precede rollback");
        let file_rollback = rollback_source
            .find("rollback_local_workspace_files(")
            .expect("workspace files must be restored through the port adapter");
        let history_cleanup = file_rollback
            + rollback_source[file_rollback..]
                .find("if request.delete_turns")
                .expect("history cleanup must remain host-owned");
        let history_event = rollback_source
            .find("conversation_turns_deleted")
            .expect("history event must remain host-projected");
        let rollback_event = rollback_source
            .find("turn_rolled_back")
            .expect("rollback event must remain host-projected");
        assert!(
            remote_guard < maintenance
                && maintenance < file_rollback
                && file_rollback < history_cleanup
                && history_cleanup < history_event
                && history_event < rollback_event,
            "Desktop rollback must preserve remote, maintenance, files, history, and event order"
        );

        let sdk_source = include_str!("../../../../crates/execution/agent-runtime/src/sdk.rs");
        assert!(!sdk_source.contains("LocalWorkspaceSnapshot"));
    }

    #[test]
    fn desktop_interaction_runtime_does_not_claim_unimplemented_product_services() {
        let runtime_source = include_str!("mod.rs");
        let product_assembler = ["Product", "Assembler"].concat();
        let runtime_services = ["Runtime", "Services"].concat();
        let desktop_services_provider = ["DesktopRuntime", "ServicesProvider"].concat();

        assert!(!runtime_source.contains(&product_assembler));
        assert!(!runtime_source.contains(&runtime_services));
        assert!(!runtime_source.contains(&desktop_services_provider));
    }

    #[test]
    fn desktop_session_writes_reuse_the_coordinator_ownership_owner() {
        let application = include_str!("session_application.rs");
        let app_entrypoint = include_str!("../lib.rs");
        let agentic_api = include_str!("../api/agentic_api.rs");
        let remote_connect_api = include_str!("../api/remote_connect_api.rs");
        let snapshot_api = include_str!("../api/snapshot_service.rs");
        let workspace_activation = include_str!("../api/workspace_activation.rs");

        assert!(
            !workspace_activation.contains("initialize_snapshot_manager_for_workspace"),
            "read-only workspace activation must not attach the snapshot Runtime"
        );

        assert!(
            app_entrypoint.contains("CoreRuntimeOwnership::embedded"),
            "Desktop composition must inject one lazy multi-workspace Core owner"
        );
        assert!(
            application
                .matches(".ensure_workspace_runtime_ownership(")
                .count()
                == 1,
            "Desktop application must delegate ownership to one Coordinator gate"
        );
        assert!(
            application
                .matches("self.ensure_runtime_ownership(&scope)")
                .count()
                >= 6,
            "Desktop attach and mutation paths must reuse one application helper"
        );
        assert!(
            !application.contains("RuntimeOwnershipKey")
                && !application.contains("WorkspaceRuntimeOwnership"),
            "Desktop application must not duplicate ownership primitives"
        );

        let create_session = agentic_api
            .split_once("pub async fn create_session")
            .expect("create_session")
            .1
            .split_once("pub async fn update_session_model")
            .expect("create_session boundary")
            .0;
        assert!(
            create_session.contains("session_application()")
                && create_session.contains("ensure_workspace_runtime_ownership"),
            "Desktop session creation must validate remote facts through the shared application scope resolver"
        );

        let view = application
            .split_once("pub(crate) async fn restore_session_view")
            .expect("view restore")
            .1
            .split_once("pub(crate) async fn restore_session_with_turns")
            .expect("view restore boundary")
            .0;
        assert!(
            !view.contains("ensure_workspace_runtime_ownership"),
            "read-only view restore must remain available without acquiring runtime ownership"
        );

        for (mutation, end) in [
            ("if is_idempotent_review_create", "let config = request"),
            (
                "pub async fn set_session_memory_mode",
                "pub async fn clear_session_thread_goal",
            ),
        ] {
            let source = agentic_api
                .split_once(mutation)
                .unwrap_or_else(|| panic!("missing Desktop mutation: {mutation}"))
                .1
                .split_once(end)
                .unwrap_or_else(|| panic!("missing Desktop mutation boundary: {end}"))
                .0;
            assert!(
                source.contains("ensure_workspace_runtime_ownership")
                    || source.contains("ensure_session_runtime_ownership"),
                "Desktop mutation {mutation} must pass through the Core ownership owner"
            );
        }

        for (mutation, end) in [
            (
                "pub async fn account_import_remote_sessions",
                "pub async fn account_fetch_session_turns",
            ),
            (
                "pub async fn account_fetch_session_turns",
                "pub async fn account_execute_on_device",
            ),
            (
                "async fn import_session_bundle",
                "async fn pull_and_reconcile",
            ),
        ] {
            let source = remote_connect_api
                .split_once(mutation)
                .unwrap_or_else(|| panic!("missing relay mutation: {mutation}"))
                .1
                .split_once(end)
                .unwrap_or_else(|| panic!("missing relay mutation boundary: {end}"))
                .0;
            assert!(
                source.contains("ensure_workspace_runtime_ownership"),
                "relay mutation {mutation} must pass through the Core ownership owner"
            );
        }

        for mutation in [
            "pub async fn initialize_snapshot",
            "pub async fn record_file_change",
            "pub async fn rollback_session",
            "pub async fn rollback_to_turn",
            "pub async fn accept_session",
            "pub async fn accept_file",
            "pub async fn reject_file",
            "pub async fn accept_operation",
            "pub async fn reject_operation",
        ] {
            let source = snapshot_api
                .split_once(mutation)
                .unwrap_or_else(|| panic!("missing snapshot mutation: {mutation}"))
                .1
                .split_once("#[tauri::command]")
                .unwrap_or_else(|| panic!("missing snapshot mutation boundary: {mutation}"))
                .0;
            assert!(
                source.contains("ensure_local_runtime_ownership"),
                "snapshot mutation {mutation} must acquire ownership before side effects"
            );
        }
    }
}
