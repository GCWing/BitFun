//! Event system module

pub mod emitter;
#[cfg(feature = "runtime-services")]
pub mod event_system;

pub use emitter::EventEmitter;
#[cfg(feature = "runtime-services")]
pub use event_system::BackendEventSystem as BackendEventManager;
#[cfg(feature = "runtime-services")]
pub use event_system::{
    emit_global_event, get_global_event_system, BackendEvent, BackendEventSystem,
};
