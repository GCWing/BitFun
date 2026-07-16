//! OpenCode-compatible plugin adapter.
//!
//! The production surface is intentionally small: load OpenCode-compatible
//! managed package content and optional activation authority as a Plugin Runtime
//! Host adapter plus typed dispatch targets. The adapter does not execute
//! JavaScript, install npm packages, or depend on a user-local `opencode` CLI.

mod command_source;
mod source_adapter;

pub use command_source::{OpenCodeCommandProvider, OpenCodeCommandProviderOptions};
pub use source_adapter::load_opencode_package_adapter;
