//! Bootstrap WorkspaceService / FileSystemService / DialogScheduler for Peer Host.

use std::sync::Arc;

use anyhow::{Context, Result};
use bitfun_core::service::filesystem::FileSystemServiceFactory;
use bitfun_core::service::workspace::{self, WorkspaceService};

use crate::runtime::CliRuntimeContext;

use super::fanout::start_peer_event_fanout;
use super::state::{set_peer_host_state, try_peer_host_state, PeerHostState, PeerTurnTracker};

/// Ensure Peer Host services are ready. Idempotent.
pub(crate) async fn ensure_peer_host_ready(runtime: &CliRuntimeContext) -> Result<()> {
    if try_peer_host_state().is_some() {
        return Ok(());
    }

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
        agent_runtime: runtime.agent_runtime().clone(),
        local_workspace_snapshot: runtime.local_workspace_snapshot().clone(),
        compatibility: runtime.compatibility().clone(),
        agent_events: runtime.agent_events().clone(),
        turns: PeerTurnTracker::new(),
        workspace_service,
        filesystem_service,
    };

    if set_peer_host_state(state.clone()).is_err() {
        // Another task won the race; treat as success.
        return Ok(());
    }

    start_peer_event_fanout(state);
    tracing::info!("CLI peer host services ready");
    Ok(())
}
