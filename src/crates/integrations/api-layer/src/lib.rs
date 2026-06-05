/// BitFun API Layer
///
/// Platform-agnostic business logic layer, used by:
/// - CLI (apps/cli)
/// - Tauri Desktop (apps/desktop)
/// - Web Server (apps/server)
pub mod dto;
pub mod handlers;

pub use dto::*;
pub use handlers::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
