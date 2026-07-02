//! Browser control integration services.
//!
//! This module owns platform browser detection and CDP launch process handling.
//! Product policy, tool routing, and UI commands stay in product assembly and
//! app entrypoints.

pub mod launcher;

pub use launcher::{
    BrowserKind, BrowserLaunchOptions, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
