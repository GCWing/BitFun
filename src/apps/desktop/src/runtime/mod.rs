use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use openbitfun_agent_runtime::sdk::SessionEventJournal;
use openbitfun_agent_runtime::sdk::{AgentRuntime, PermissionRequestEvent};
use openbitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use openbitfun_core::service::remote_ssh::SSHConnectionManager;
use openbitfun_core::service::token_usage::TokenUsageService;
use openbitfun_core::service::workspace::WorkspaceService;
use tokio::sync::RwLock;

mod session_application;
mod session_host_effects;

use session_host_effects::ProductionDesktopSessionHostEffects;

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
    permission_events_started: AtomicBool,
}

impl DesktopRuntimeContext {
    pub(crate) fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
        workspace_service: Arc<WorkspaceService>,
        ssh_manager: Arc<RwLock<Option<SSHConnectionManager>>>,
        acp_client_service: Option<Arc<openbitfun_acp::AcpClientService>>,
        session_event_journal: Arc<SessionEventJournal>,
    ) -> Result<Self, String> {
        let host_effects = Arc::new(ProductionDesktopSessionHostEffects::new(acp_client_service));
        let session_application = DesktopSessionApplication::build(
            coordinator,
            scheduler,
            token_usage_service,
            workspace_service,
            ssh_manager,
            host_effects,
            session_event_journal,
        )?;
        Ok(Self {
            session_application,
            permission_events_started: AtomicBool::new(false),
        })
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        self.session_application.agent_runtime()
    }

    pub(crate) fn session_application(&self) -> &DesktopSessionApplication {
        &self.session_application
    }

    pub(crate) fn start_permission_event_forwarding(
        &self,
        app: tauri::AppHandle,
    ) -> Result<(), openbitfun_agent_runtime::sdk::RuntimeError> {
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
        assert!(snapshot_commands.contains("begin_snapshot_history_read"));
        assert!(snapshot_commands.contains("open_snapshot_manager_for_view"));
        assert!(snapshot_commands.contains("ensure_local_snapshot_mutation_path"));

        let rollback_source = &snapshot_commands[snapshot_commands
            .find("pub async fn rollback_session_to_turn")
            .expect("rollback command must exist")..];
        let remote_guard = rollback_source
            .find("ensure_complete_rollback_supported")
            .expect("remote rollback guard must remain host-owned");
        let runtime_revert = rollback_source
            .find(".rollback_session_to_turn(request)")
            .expect("rollback must delegate to the Agent Session transaction");
        assert!(remote_guard < runtime_revert);

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
            "pub async fn rollback_session",
            "pub async fn rollback_session_to_turn",
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
