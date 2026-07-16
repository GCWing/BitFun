//! Codex plugin adapter for BitFun.
//!
//! Standalone `PluginHostAdapter` for Codex `.codex-plugin/plugin.json` sources.
//! Independent from opencode-adapter — implements the same trait, managed by
//! Plugin Runtime Host as a peer.
//!
//! Public API budget: [`load_codex_compatible_adapter`].

mod source_adapter;
mod manifest;
pub mod discovery;
mod event_map;

pub use source_adapter::load_codex_compatible_adapter;
pub use discovery::LoadedCodexPlugin;
pub use discovery::PluginDiscovery;
