//! Core-compatible event layer facade.
//!
//! Provider-neutral queue and routing owners live in `openbitfun-agent-runtime`.

pub mod queue {
    pub use openbitfun_agent_runtime::event_queue::*;
}

pub mod router {
    pub use openbitfun_agent_runtime::event_router::*;
}

pub mod types;

pub use queue::*;
pub use router::*;
pub use types::*;
