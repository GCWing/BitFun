//! Codex plugin adapter for BitFun.
//!
//! Wraps the OpenCode adapter to additionally discover and translate
//! Codex `.codex-plugin/plugin.json` sources. The adapter respects the
//! BitFun adapter boundary rules:
//!
//! - Only translates manifests to typed candidates (no plugin execution).
//! - Codex hook event names are mapped to OpenCode lifecycle events.
//! - Skill, MCP, and hooks wiring belongs in the assembly layer.
//!
//! Public API budget: [`load_codex_workspace_adapter`].

mod source_adapter;
mod manifest;
pub mod discovery;
mod event_map;

pub use source_adapter::load_codex_workspace_adapter;
