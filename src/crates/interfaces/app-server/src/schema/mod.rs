//! JSON-RPC wire contract for the BitFun app-server surface.
//!
//! Schema types are grouped by product domain while this module re-exports the
//! complete contract. Existing consumers can continue importing types from
//! `bitfun_app_server::schema` without depending on the internal layout.

mod agent;
mod app;
mod config;
mod events;
mod git;
mod i18n;
mod permission;
mod session;

pub use agent::*;
pub use app::*;
pub use config::*;
pub use events::*;
pub use git::*;
pub use i18n::*;
pub use permission::*;
pub use session::*;
