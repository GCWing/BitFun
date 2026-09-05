//! OpenBitFun Agent Client Protocol integration.
//!
//! This crate owns the ACP client and server surfaces over OpenBitFun's assembled
//! runtime. Product hosts select the additive `client` and `server` features
//! explicitly; the compatibility default enables both roles.

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
mod runtime;
#[cfg(feature = "server")]
mod server;

pub use agent_client_protocol as protocol;
#[cfg(feature = "client")]
pub use client::AcpClientService;
#[cfg(feature = "server")]
pub use runtime::OpenBitFunAcpRuntime;
#[cfg(feature = "server")]
pub use server::AcpServer;
