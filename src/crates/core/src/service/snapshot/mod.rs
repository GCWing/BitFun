pub mod events;
pub mod file_lock_manager;
pub mod isolation_manager;
pub mod manager;
pub mod service;
pub mod snapshot_core;
pub mod snapshot_system;
pub mod types;

pub use events::{
    emit_snapshot_event, emit_snapshot_session_event, initialize_snapshot_event_emitter,
    SnapshotEvent, SnapshotEventEmitter,
};
pub use manager::{
    ensure_snapshot_manager_for_workspace, get_or_create_snapshot_manager,
    get_snapshot_manager_for_workspace, get_snapshot_wrapped_tools,
    initialize_snapshot_manager_for_workspace, wrap_tool_for_snapshot_tracking, SnapshotManager,
};
pub use service::{SnapshotService, SystemStats};
pub use snapshot_core::{FileChangeEntry, FileChangeQueue, SessionStats, SnapshotCore};
pub use types::*;
