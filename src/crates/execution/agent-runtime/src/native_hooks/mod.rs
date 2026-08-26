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

mod call;
mod engine;
mod handler;
mod kind;
mod output;
mod payload;
mod registry;
mod settings;

pub use call::{HookCall, HookCallPayload};
pub use engine::{AgentHookEngine, PluginHookDispatchResult, MAX_HOOK_MODEL_OUTPUT_BYTES};
pub use handler::{
    BuiltinHookExecutor, HookHandler, HookHandlerResult, PluginHookCall, PluginHookExecutor,
    PluginHookGenerationIdentity, PluginHookResult, RuntimeHookRegistration,
};
pub use kind::{RuntimeHookKind, RuntimeHookSource};
pub use output::{AgentHookOutcome, AgentHookPermissionOutcome};
pub use payload::{
    AgentHookEventPayload, AgentHookPayload, AgentHookPayloadCommon, AgentHookPermissionMode,
};
pub use registry::{
    RuntimeHookActivation, RuntimeHookCommitToken, RuntimeHookErrorPolicy, RuntimeHookPlan,
    RuntimeHookRegistry, RuntimeHookRegistryBuildError, RuntimeHookRegistryBuilder,
    RuntimeHookRegistryError,
};
pub use settings::{
    AgentHookEvent, AgentHookHandler, AgentHookMatcher, AgentHookRule, AgentHookScope,
    AgentHookSettings, AgentHookSettingsIssue, AgentHookSettingsLayer, MAX_HOOKS_FILE_BYTES,
    MAX_HOOK_HANDLERS,
};
