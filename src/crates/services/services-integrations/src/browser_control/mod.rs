//! Browser control integration services.
//!
//! This module owns platform browser detection, CDP endpoint HTTP handling, and
//! CDP launch process handling.
//! Product policy, tool routing, and UI commands stay in product assembly and
//! app entrypoints.

pub mod cdp;
pub mod launcher;

pub use cdp::{CdpEndpointProvider, CdpPageInfo, CdpVersionInfo};
pub use launcher::{
    BrowserKind, BrowserLaunchOptions, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
