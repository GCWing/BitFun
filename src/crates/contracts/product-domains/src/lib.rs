//! Product domain owner crate.
//!
//! Product subdomains live here when they can be compiled without depending on
//! the full BitFun core runtime assembly.

pub mod canvas;
pub mod tool_permissions;

#[cfg(feature = "external-sources")]
pub mod external_integration_policy;

#[cfg(feature = "external-sources")]
pub mod external_source_control;

#[cfg(feature = "external-sources")]
pub mod external_sources;

#[cfg(feature = "external-sources")]
pub mod external_subagents;

#[cfg(feature = "plugin-source")]
pub mod plugin_source;

#[cfg(feature = "miniapp")]
pub mod miniapp;

#[cfg(feature = "function-agents")]
pub mod function_agents;
