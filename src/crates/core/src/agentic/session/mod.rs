//! Session Management Layer
//!
//! Provides session lifecycle management and context management.

pub mod compression;
pub mod context_store;
pub mod evidence_ledger;
pub mod file_read_state;
pub mod prompt_cache;
pub mod session_manager;
pub mod session_store_port;
pub mod turn_skill_agent_snapshot_store;

pub use compression::*;
pub use context_store::*;
pub use evidence_ledger::*;
pub use file_read_state::*;
pub use prompt_cache::*;
pub use session_manager::*;
pub use session_store_port::*;
pub use turn_skill_agent_snapshot_store::*;

pub use bitfun_runtime_ports::{
    SessionStorageKind, SessionStoragePathRequest, SessionStoragePathResolution,
    SessionTurnLoadTiming, SessionViewRestoreRequest, SessionViewRestoreTiming,
};
