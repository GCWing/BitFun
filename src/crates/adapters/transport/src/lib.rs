pub mod adapters;
pub mod emitter;
/// BitFun Transport Layer
///
/// Event delivery abstraction used by current product hosts.
pub mod traits;

pub use emitter::TransportEmitter;
pub use traits::TransportAdapter;

#[cfg(feature = "tauri-adapter")]
pub use adapters::TauriTransportAdapter;
