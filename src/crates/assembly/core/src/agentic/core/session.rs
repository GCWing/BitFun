//! Compatibility facade for runtime-owned session facts.

pub use bitfun_agent_runtime::session::{
    sanitize_persisted_session_state, CompressionState, PersistedSessionStateFile, Session,
    SessionConfig, SessionKind, SessionSummary,
};
