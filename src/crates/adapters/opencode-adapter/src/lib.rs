//! OpenCode-compatible plugin adapter.
//!
//! The production surface is intentionally small: load OpenCode-compatible
//! workspace sources plus existing `PluginSourceRef` trust snapshots as a Plugin
//! Runtime Host adapter that exposes source facts, diagnostics, and trust-gated
//! provider candidates. It does not execute JavaScript, install npm packages,
//! or depend on a user-local `opencode` CLI.

mod source_adapter;

pub use source_adapter::load_opencode_workspace_adapter;
