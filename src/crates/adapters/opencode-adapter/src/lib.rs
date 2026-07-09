//! OpenCode-compatible plugin adapter.
//!
//! The production surface is intentionally small: load OpenCode-compatible
//! workspace sources as a projection-only Plugin Runtime Host adapter. It does
//! not execute JavaScript, install npm packages, or depend on a user-local
//! `opencode` CLI.

mod source_adapter;

pub use source_adapter::load_opencode_workspace_adapter;
