//! Product domain owner crate.
//!
//! Product subdomains live here when they can be compiled without depending on
//! the full BitFun core runtime assembly.

#[cfg(feature = "miniapp")]
pub mod miniapp;

#[cfg(feature = "function-agents")]
pub mod function_agents;
