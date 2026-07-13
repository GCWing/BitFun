//! Portable post-call hook routing decisions.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Hook categories that concrete runtime integrations may execute after a
/// successful tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuntimeHookKind {
    SuccessfulToolPostCall,
    DeepReviewSharedContextToolUse,
    BehaviorGuard,
}

pub const fn successful_tool_post_call_hooks() -> [RuntimeHookKind; 2] {
    [
        RuntimeHookKind::DeepReviewSharedContextToolUse,
        RuntimeHookKind::BehaviorGuard,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeHookErrorPolicy {
    FailTurn,
    SkipHook,
    DenyTool,
    RecordWarning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult {
    Continue,
    Abort {
        reason: String,
        fix_instruction: String,
        max_retries: u32,
    },
}

impl HookResult {
    pub fn is_abort(&self) -> bool {
        matches!(self, HookResult::Abort { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHookPlan {
    id: String,
    kind: RuntimeHookKind,
    order: u16,
    timeout_millis: u64,
    error_policy: RuntimeHookErrorPolicy,
}

impl RuntimeHookPlan {
    pub fn new(id: impl Into<String>, kind: RuntimeHookKind) -> Self {
        Self {
            id: id.into(),
            kind,
            order: 100,
            timeout_millis: 1_000,
            error_policy: RuntimeHookErrorPolicy::RecordWarning,
        }
    }

    pub fn with_order(mut self, order: u16) -> Self {
        self.order = order;
        self
    }

    pub fn with_timeout_millis(mut self, timeout_millis: u64) -> Self {
        self.timeout_millis = timeout_millis;
        self
    }

    pub fn with_error_policy(mut self, error_policy: RuntimeHookErrorPolicy) -> Self {
        self.error_policy = error_policy;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn kind(&self) -> RuntimeHookKind {
        self.kind
    }

    pub const fn order(&self) -> u16 {
        self.order
    }

    pub const fn timeout_millis(&self) -> u64 {
        self.timeout_millis
    }

    pub const fn error_policy(&self) -> RuntimeHookErrorPolicy {
        self.error_policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeHookRegistryBuildError {
    #[error("runtime hook id must not be empty")]
    EmptyHookId,
    #[error("runtime hook {hook_id} must declare a non-zero timeout")]
    InvalidTimeoutMillis { hook_id: String },
    #[error("duplicate runtime hook id {hook_id}")]
    DuplicateHookId { hook_id: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeHookRegistryBuilder {
    hooks: Vec<RuntimeHookPlan>,
}

impl RuntimeHookRegistryBuilder {
    pub fn register(mut self, hook: RuntimeHookPlan) -> Self {
        self.hooks.push(hook);
        self
    }

    pub fn build(mut self) -> Result<RuntimeHookRegistry, RuntimeHookRegistryBuildError> {
        let mut hook_ids = HashSet::new();
        for hook in &self.hooks {
            if hook.id.trim().is_empty() {
                return Err(RuntimeHookRegistryBuildError::EmptyHookId);
            }
            if hook.timeout_millis == 0 {
                return Err(RuntimeHookRegistryBuildError::InvalidTimeoutMillis {
                    hook_id: hook.id.clone(),
                });
            }
            if !hook_ids.insert(hook.id.clone()) {
                return Err(RuntimeHookRegistryBuildError::DuplicateHookId {
                    hook_id: hook.id.clone(),
                });
            }
        }
        self.hooks.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(RuntimeHookRegistry { hooks: self.hooks })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeHookRegistry {
    hooks: Vec<RuntimeHookPlan>,
}

impl RuntimeHookRegistry {
    pub fn builder() -> RuntimeHookRegistryBuilder {
        RuntimeHookRegistryBuilder::default()
    }

    pub fn hooks(&self) -> &[RuntimeHookPlan] {
        &self.hooks
    }
}

pub trait SuccessfulToolPostCallHookExecutor<C> {
    fn record_deep_review_shared_context_tool_use(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &C,
    );

    fn behavior_guard(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &C,
    ) -> HookResult;
}

pub fn run_successful_tool_post_call_hooks<C, E>(
    tool_name: &str,
    input: &Value,
    context: &C,
    executor: &mut E,
) -> HookResult
where
    E: SuccessfulToolPostCallHookExecutor<C>,
{
    for hook in successful_tool_post_call_hooks() {
        match hook {
            RuntimeHookKind::DeepReviewSharedContextToolUse => {
                executor.record_deep_review_shared_context_tool_use(tool_name, input, context);
            }
            RuntimeHookKind::BehaviorGuard => {
                let result = executor.behavior_guard(tool_name, input, context);
                if result.is_abort() {
                    return result;
                }
            }
            RuntimeHookKind::SuccessfulToolPostCall => {}
        }
    }
    HookResult::Continue
}

#[derive(Debug, Clone, Copy)]
pub struct DeepReviewSharedContextToolUseFacts<'a> {
    pub tool_name: &'a str,
    pub input: &'a Value,
    pub custom_data: &'a HashMap<String, Value>,
    pub workspace_root: Option<&'a Path>,
    pub is_remote: bool,
    pub agent_type: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewSharedContextToolUseRecord {
    pub parent_turn_id: String,
    pub subagent_type: String,
    pub tool_name: String,
    pub measured_path: String,
}

pub fn resolve_deep_review_shared_context_tool_use(
    facts: DeepReviewSharedContextToolUseFacts<'_>,
) -> Option<DeepReviewSharedContextToolUseRecord> {
    if !facts.tool_name.eq_ignore_ascii_case("Read")
        && !facts.tool_name.eq_ignore_ascii_case("GetFileDiff")
    {
        return None;
    }
    if !custom_data_str(facts.custom_data, "deep_review_subagent_role")
        .is_some_and(|role| role.eq_ignore_ascii_case("reviewer"))
    {
        return None;
    }
    let parent_turn_id = custom_data_str(facts.custom_data, "deep_review_parent_dialog_turn_id")?;
    let file_path = facts
        .input
        .get("file_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if is_bitfun_runtime_uri(file_path) {
        return None;
    }

    let measured_path = if facts.is_remote {
        None
    } else {
        facts
            .workspace_root
            .and_then(|workspace_root| git_relative_path(workspace_root, file_path))
    }
    .unwrap_or_else(|| file_path.to_string());
    let subagent_type = custom_data_str(facts.custom_data, "deep_review_subagent_type")
        .or(facts.agent_type)
        .unwrap_or("unknown");

    Some(DeepReviewSharedContextToolUseRecord {
        parent_turn_id: parent_turn_id.to_string(),
        subagent_type: subagent_type.to_string(),
        tool_name: facts.tool_name.to_string(),
        measured_path,
    })
}

fn custom_data_str<'a>(custom_data: &'a HashMap<String, Value>, key: &str) -> Option<&'a str> {
    custom_data
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn git_relative_path(workspace_root: &Path, path: &str) -> Option<String> {
    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(workspace_root).ok()?
    } else {
        path.strip_prefix(workspace_root).unwrap_or(path)
    };

    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn is_bitfun_runtime_uri(path: &str) -> bool {
    path.trim().starts_with("bitfun://runtime/")
}
