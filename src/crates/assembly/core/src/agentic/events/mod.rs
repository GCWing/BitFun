//! Core-compatible event layer facade.
//!
//! Provider-neutral queue and routing owners live in `bitfun-agent-runtime`.

pub mod queue {
    pub use bitfun_agent_runtime::event_queue::*;
}

pub mod router {
    pub use bitfun_agent_runtime::event_router::*;
}

pub mod types;

pub use queue::*;
pub use router::*;
pub use types::*;
