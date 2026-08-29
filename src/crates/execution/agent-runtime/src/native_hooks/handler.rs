//! Executable hook handler variants.

use super::call::HookCall;
use super::kind::{RuntimeHookKind, RuntimeHookSource};
use super::registry::RuntimeHookPlan;
use super::settings::{AgentHookHandler, AgentHookMatcher};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub enum HookHandler {
    Command(AgentHookHandler),
    Plugin {
        executor: Arc<dyn PluginHookExecutor>,
        hook_name: String,
        instance_id: String,
        generation_key: String,
        revision: String,
    },
    Builtin {
        executor: Arc<dyn BuiltinHookExecutor>,
    },
}

impl std::fmt::Debug for HookHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(handler) => f.debug_tuple("Command").field(handler).finish(),
            Self::Plugin {
                hook_name,
                instance_id,
                generation_key,
                revision,
                ..
            } => f
                .debug_struct("Plugin")
                .field("hook_name", hook_name)
                .field("instance_id", instance_id)
                .field("generation_key", generation_key)
                .field("revision", revision)
                .finish_non_exhaustive(),
            Self::Builtin { .. } => f.debug_struct("Builtin").finish_non_exhaustive(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HookHandlerResult {
    pub warnings: Vec<String>,
    pub block_reason: Option<String>,
    pub additional_context: Vec<String>,
}

#[async_trait]
pub trait BuiltinHookExecutor: Send + Sync {
    async fn execute(&self, call: &HookCall) -> HookHandlerResult;
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginHookCall {
    pub instance_id: String,
    pub workspace_scope: String,
    pub generation_key: String,
    pub revision: String,
    pub hook_name: String,
    pub input: Value,
    pub output: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginHookResult {
    pub instance_id: String,
    pub generation_key: String,
    pub revision: String,
    pub hook_name: String,
    pub input: Value,
    pub output: Value,
}

#[async_trait]
pub trait PluginHookExecutor: Send + Sync {
    async fn execute(&self, call: PluginHookCall) -> Result<PluginHookResult, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHookGenerationIdentity {
    pub instance_id: String,
    pub generation_key: String,
    pub revision: String,
}

#[derive(Clone, Debug)]
pub struct RuntimeHookRegistration {
    pub plan: RuntimeHookPlan,
    pub handler: HookHandler,
    pub matcher: AgentHookMatcher,
    pub workspace_scope: Option<String>,
}

impl RuntimeHookRegistration {
    pub fn new(plan: RuntimeHookPlan, handler: HookHandler, matcher: AgentHookMatcher) -> Self {
        Self {
            plan,
            handler,
            matcher,
            workspace_scope: None,
        }
    }

    pub fn with_workspace_scope(mut self, workspace_scope: impl Into<String>) -> Self {
        self.workspace_scope = Some(workspace_scope.into());
        self
    }

    pub fn command(
        id: impl Into<String>,
        kind: RuntimeHookKind,
        source: RuntimeHookSource,
        handler: AgentHookHandler,
        matcher: AgentHookMatcher,
    ) -> Self {
        Self::new(
            RuntimeHookPlan::new(id, kind, source),
            HookHandler::Command(handler),
            matcher,
        )
    }

    pub fn plugin(
        plan: RuntimeHookPlan,
        hook_name: impl Into<String>,
        instance_id: impl Into<String>,
        generation_key: impl Into<String>,
        revision: impl Into<String>,
        executor: Arc<dyn PluginHookExecutor>,
        matcher: AgentHookMatcher,
    ) -> Self {
        Self::new(
            plan,
            HookHandler::Plugin {
                executor,
                hook_name: hook_name.into(),
                instance_id: instance_id.into(),
                generation_key: generation_key.into(),
                revision: revision.into(),
            },
            matcher,
        )
    }
}
