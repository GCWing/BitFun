//! Native BitFun agent lifecycle hooks.
//!
//! This module owns the portable hook engine that executes user-configured
//! command hooks at agent lifecycle events. The configuration document, event
//! names, process interface (stdin JSON payload, exit-code semantics, stdout
//! decision schema), matcher semantics, and timeout defaults are kept
//! consistent with Codex hooks so users can reuse existing hook scripts.
//!
//! The engine is host-independent: it receives already-loaded settings layers
//! and fully-built payloads, and never resolves BitFun config paths itself.
//! Config discovery, scope gating, and dispatch-site integration live in
//! `bitfun-core` (`native_hooks` wiring).
//!
//! Distinct from:
//! - `post_call_hooks`: internal compiled-in Rust hooks (not user-configured).
//! - the external hook catalog (`bitfun-product-domains`): read-only
//!   inspection of other AI applications' hook configuration.

mod engine;
mod output;
mod payload;
mod settings;

pub use engine::{AgentHookEngine, MAX_HOOK_MODEL_OUTPUT_BYTES};
pub use output::{AgentHookOutcome, AgentHookPermissionOutcome};
pub use payload::{
    AgentHookEventPayload, AgentHookPayload, AgentHookPayloadCommon, AgentHookPermissionMode,
};
pub use settings::{
    AgentHookEvent, AgentHookHandler, AgentHookMatcher, AgentHookRule, AgentHookScope,
    AgentHookSettings, AgentHookSettingsIssue, AgentHookSettingsLayer, MAX_HOOKS_FILE_BYTES,
    MAX_HOOK_HANDLERS,
};
