//! Bootstrap WorkspaceService / FileSystemService / DialogScheduler for Peer Host.

use std::sync::Arc;

use anyhow::{Context, Result};
use bitfun_core::agentic::coordination::{self, DialogScheduler};
use bitfun_core::agentic::system::AgenticSystem;
use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::filesystem::FileSystemServiceFactory;
use bitfun_core::service::workspace::{self, WorkspaceService};

use super::fanout::start_peer_event_fanout;
use super::state::{set_peer_host_state, try_peer_host_state, PeerHostState};

/// Ensure Peer Host services are ready. Idempotent.
pub(crate) async fn ensure_peer_host_ready(agentic: &AgenticSystem) -> Result<()> {
    if try_peer_host_state().is_some() {
        return Ok(());
    }

    let path_manager = try_get_path_manager_arc().context("path manager")?;
    let persistence = Arc::new(
        bitfun_core::agentic::persistence::PersistenceManager::new(path_manager)
            .context("persistence manager")?,
    );

    let scheduler = if let Some(existing) = coordination::get_global_scheduler() {
        existing
    } else {
        let session_manager = agentic.coordinator.get_session_manager().clone();
        let scheduler = DialogScheduler::new(agentic.coordinator.clone(), session_manager);
        agentic
            .coordinator
            .set_scheduler_notifier(scheduler.outcome_sender());
        agentic
            .coordinator
            .set_round_injection_source(scheduler.round_injection_monitor());
        coordination::set_global_scheduler(scheduler.clone());
        scheduler
    };

    let workspace_service = if let Some(existing) = workspace::get_global_workspace_service() {
        existing
    } else {
        let service = Arc::new(
            WorkspaceService::new()
                .await
                .context("WorkspaceService::new")?,
        );
        workspace::set_global_workspace_service(service.clone());
        service
    };

    let filesystem_service = Arc::new(FileSystemServiceFactory::create_default());

    let state = PeerHostState {
        coordinator: agentic.coordinator.clone(),
        scheduler,
        event_queue: agentic.event_queue.clone(),
        workspace_service,
        filesystem_service,
        persistence,
    };

    if set_peer_host_state(state.clone()).is_err() {
        // Another task won the race; treat as success.
        return Ok(());
    }

    start_peer_event_fanout(state.event_queue.clone());
    tracing::info!("CLI peer host services ready");
    Ok(())
}
