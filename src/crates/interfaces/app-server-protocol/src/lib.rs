//! Behavior-light wire contracts for BitFun App Server clients and hosts.
//!
//! This crate intentionally has no dependency on Core, Runtime implementations,
//! services, product assembly, or a UI framework. Server adapters translate
//! these wire DTOs to owner types at the interface boundary.

pub mod app;
pub mod error;
pub mod event;
pub mod method;
pub mod role;
pub mod transport;
pub mod tui;

pub use role::{AppClient, AppServer};

/// Current App Server protocol version.
pub const PROTOCOL_VERSION: u32 = 3;

/// Oldest protocol version this implementation accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 2;
