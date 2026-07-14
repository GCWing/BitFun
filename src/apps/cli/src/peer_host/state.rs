//! Shared Peer Host service handles.

use std::sync::{Arc, OnceLock};

use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use bitfun_core::agentic::events::EventQueue;
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::service::filesystem::FileSystemService;
use bitfun_core::service::workspace::WorkspaceService;

#[derive(Clone)]
pub(crate) struct PeerHostState {
    pub(crate) coordinator: Arc<ConversationCoordinator>,
    pub(crate) scheduler: Arc<DialogScheduler>,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) workspace_service: Arc<WorkspaceService>,
    pub(crate) filesystem_service: Arc<FileSystemService>,
    pub(crate) persistence: Arc<PersistenceManager>,
}

static PEER_HOST_STATE: OnceLock<PeerHostState> = OnceLock::new();

pub(crate) fn set_peer_host_state(state: PeerHostState) -> Result<(), PeerHostState> {
    PEER_HOST_STATE.set(state)
}

pub(crate) fn try_peer_host_state() -> Option<&'static PeerHostState> {
    PEER_HOST_STATE.get()
}

pub(crate) fn peer_host_state() -> Result<&'static PeerHostState, String> {
    try_peer_host_state().ok_or_else(|| "CLI peer host is not initialized".to_string())
}
