//! OpenCode-compatible plugin adapter.
//!
//! The production surface is intentionally small: load OpenCode-compatible
//! managed package content as a Plugin Runtime Host adapter that exposes source
//! facts, diagnostics, and an unactivated status. Candidate mapping remains
//! private until a separately reviewed Host activation path exists. The adapter
//! does not execute JavaScript, install npm packages, or depend on a user-local
//! `opencode` CLI.

mod source_adapter;

pub use source_adapter::load_opencode_package_adapter;
