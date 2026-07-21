use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bitfun_agent_runtime::sdk::{AgentRuntime, PermissionRequestEvent};
use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use bitfun_core::product_runtime::CoreLocalWorkspaceSnapshot;
use bitfun_core::service::remote_ssh::SSHConnectionManager;
use bitfun_core::service::token_usage::TokenUsageService;
use bitfun_core::service::workspace::WorkspaceService;
use bitfun_runtime_ports::LocalWorkspaceSnapshotPort;
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
    local_workspace_snapshot: Arc<dyn LocalWorkspaceSnapshotPort>,
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
    ) -> Result<Self, String> {
        let host_effects = Arc::new(ProductionDesktopSessionHostEffects::new(acp_client_service));
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
            4,
            "only raw turn save, transcript export, and the two excluded bulk operations keep direct persistence"
        );

        let snapshot_commands = include_str!("../api/snapshot_service.rs");
        assert_eq!(
            snapshot_commands
                .matches(".local_workspace_snapshot()")
                .count(),
            3,
            "only file listing, typed stats, and workspace rollback use the local owner port"
        );
        assert!(snapshot_commands.contains("is_remote_path(&request.workspace_path).await"));

        let rollback_source = &snapshot_commands[snapshot_commands
            .find("pub async fn rollback_to_turn")
            .expect("rollback command must exist")..];
        let remote_guard = rollback_source
            .find("if is_remote_path(&request.workspace_path).await")
            .expect("remote rollback guard must remain host-owned");
        let cancellation = rollback_source
            .find("cancel_active_turn_for_session")
            .expect("active-turn cancellation must precede rollback");
        let file_rollback = rollback_source
            .find("rollback_local_workspace_files(")
            .expect("workspace files must be restored through the port adapter");
        let history_cleanup = rollback_source
            .find("if request.delete_turns")
            .expect("history cleanup must remain host-owned");
        let history_event = rollback_source
            .find("conversation_turns_deleted")
            .expect("history event must remain host-projected");
        let rollback_event = rollback_source
            .find("turn_rolled_back")
            .expect("rollback event must remain host-projected");
        assert!(
            remote_guard < cancellation
                && cancellation < file_rollback
                && file_rollback < history_cleanup
                && history_cleanup < history_event
                && history_event < rollback_event,
            "Desktop rollback must preserve remote, cancellation, files, history, and event order"
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
}
