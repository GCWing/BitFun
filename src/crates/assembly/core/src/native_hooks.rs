//! Product wiring for native BitFun agent hooks.
//!
//! This module connects the portable hook engine
//! (`bitfun_agent_runtime::native_hooks`) to BitFun configuration and the
//! agent runtime dispatch sites:
//!
//! - Settings discovery: user scope `~/.config/bitfun/config/hooks.json`
//!   plus project scope `{project}/.bitfun/config/hooks.json`, both using the
//!   Codex-compatible `hooks.json` document schema.
//! - Gating: `hooks.enabled` and `hooks.project_hooks_enabled` in the app
//!   settings document. Project hooks are disabled by default because they
//!   execute commands declared inside the checked-out repository.
//! - Dispatch: typed helpers per lifecycle event, called from the
//!   conversation coordinator, execution engine, and tool pipeline.
//!
//! Hooks always execute on the local host. Remote workspaces skip hook
//! dispatch because the payload `cwd` and the hook process would disagree
//! about the filesystem they describe.

use crate::infrastructure::try_get_path_manager_arc;
use crate::service::config::get_global_config_service;
pub use crate::service::config::types::AgentHooksConfig;
use bitfun_agent_runtime::native_hooks::{
    AgentHookEngine, AgentHookEvent, AgentHookEventPayload, AgentHookOutcome, AgentHookPayload,
    AgentHookPayloadCommon, AgentHookPermissionMode, AgentHookPermissionOutcome, AgentHookScope,
    AgentHookSettings, AgentHookSettingsLayer, MAX_HOOKS_FILE_BYTES,
};
use dashmap::DashMap;
use log::{debug, info, warn};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

const MAX_CACHED_WORKSPACE_ENGINES: usize = 32;
const MAX_PENDING_CONTEXT_SESSIONS: usize = 1024;

/// Everything a dispatch site knows about the running session.
#[derive(Debug, Clone, Copy)]
pub struct NativeHookSessionFacts<'a> {
    pub session_id: &'a str,
    /// Present for turn-scoped events.
    pub turn_id: Option<&'a str>,
    pub workspace_root: Option<&'a Path>,
    pub is_remote_workspace: bool,
    pub model: &'a str,
    /// Maps to payload `permission_mode`: `bypassPermissions` when the turn
    /// auto-approves permission asks, `default` otherwise.
    pub bypass_permissions: bool,
}

#[derive(Debug, Default)]
pub struct UserPromptSubmitHookDecision {
    /// The prompt must not start; the reason is shown to the caller.
    pub block_reason: Option<String>,
    /// Model-visible context to prepend to the turn.
    pub additional_context: Vec<String>,
}

#[derive(Debug, Default)]
pub struct PreToolUseHookDecision {
    /// The tool call must not run; the reason is fed back to the model.
    pub deny_reason: Option<String>,
    /// The tool call bypasses the permission prompt for this invocation.
    pub allow: bool,
    /// Replacement tool arguments (`hookSpecificOutput.updatedInput`).
    pub updated_input: Option<Value>,
}

#[derive(Debug)]
pub struct PermissionRequestHookDecision {
    pub allow: bool,
    pub message: Option<String>,
}

#[derive(Debug, Default)]
pub struct PostToolUseHookDecision {
    /// Feedback the model must see (`decision: "block"` reason).
    pub block_reason: Option<String>,
    /// Extra model-visible context (`hookSpecificOutput.additionalContext`).
    pub additional_context: Vec<String>,
}

/// SessionStart hooks run when a session is created or restored.
/// Plain stdout context is buffered and injected into the next turn.
pub async fn dispatch_session_start(facts: NativeHookSessionFacts<'_>, source: &str) {
    let Some(dispatch) = prepare(facts, AgentHookEvent::SessionStart).await else {
        return;
    };
    let outcome = dispatch
        .run(AgentHookEventPayload::SessionStart {
            source: source.to_string(),
        })
        .await;
    let mut context = outcome.additional_context.clone();
    context.retain(|entry| !entry.trim().is_empty());
    if !context.is_empty() {
        let pending = pending_session_context();
        if pending.len() < MAX_PENDING_CONTEXT_SESSIONS {
            pending
                .entry(facts.session_id.to_string())
                .or_default()
                .extend(context);
        }
    }
}

/// Drain SessionStart context buffered for this session.
pub fn take_pending_session_context(session_id: &str) -> Vec<String> {
    pending_session_context()
        .remove(session_id)
        .map(|(_, context)| context)
        .unwrap_or_default()
}

/// UserPromptSubmit hooks run before the user prompt becomes a turn. A
/// blocking decision rejects the prompt; plain stdout and
/// `additionalContext` become model-visible context for the turn.
pub async fn dispatch_user_prompt_submit(
    facts: NativeHookSessionFacts<'_>,
    prompt: &str,
) -> UserPromptSubmitHookDecision {
    let mut decision = UserPromptSubmitHookDecision::default();
    let Some(dispatch) = prepare(facts, AgentHookEvent::UserPromptSubmit).await else {
        return decision;
    };
    let outcome = dispatch
        .run(AgentHookEventPayload::UserPromptSubmit {
            prompt: prompt.to_string(),
        })
        .await;
    decision.block_reason = outcome.block_reason.clone().or(outcome.stop_reason.clone());
    decision.additional_context = outcome.additional_context.clone();
    decision
}

/// PreToolUse hooks run after tool-input validation and before permission
/// evaluation. They may deny the call, pre-approve it, or rewrite its input.
pub async fn dispatch_pre_tool_use(
    facts: NativeHookSessionFacts<'_>,
    tool_name: &str,
    tool_use_id: &str,
    tool_input: &Value,
) -> PreToolUseHookDecision {
    let mut decision = PreToolUseHookDecision::default();
    let Some(dispatch) = prepare(facts, AgentHookEvent::PreToolUse).await else {
        return decision;
    };
    let outcome = dispatch
        .run(AgentHookEventPayload::PreToolUse {
            tool_name: tool_name.to_string(),
            tool_use_id: tool_use_id.to_string(),
            tool_input: tool_input.clone(),
        })
        .await;
    decision.updated_input = outcome.updated_input.clone();
    match &outcome.permission {
        Some(AgentHookPermissionOutcome::Deny { reason }) => {
            decision.deny_reason = Some(reason.clone().unwrap_or_else(|| {
                format!("A PreToolUse hook denied the '{tool_name}' tool call.")
            }));
        }
        Some(AgentHookPermissionOutcome::Allow { .. }) => {
            decision.allow = true;
        }
        None => {}
    }
    if decision.deny_reason.is_none() {
        if let Some(reason) = outcome.block_reason.clone() {
            decision.deny_reason = Some(reason);
        } else if let Some(reason) = outcome.stop_reason.clone() {
            // `continue: false` asks to stop the turn; the closest safe
            // enforcement at this dispatch site is denying the tool call.
            warn!(
                "PreToolUse hook requested a full turn stop; denying the tool call instead: tool={}",
                tool_name
            );
            decision.deny_reason = Some(reason);
        }
    }
    if decision.deny_reason.is_some() {
        decision.allow = false;
        decision.updated_input = None;
    }
    decision
}

/// PermissionRequest hooks run when a tool call would prompt the user.
/// Returns a decision only when a hook explicitly allowed or denied.
pub async fn dispatch_permission_request(
    facts: NativeHookSessionFacts<'_>,
    tool_name: &str,
    tool_input: &Value,
) -> Option<PermissionRequestHookDecision> {
    let dispatch = prepare(facts, AgentHookEvent::PermissionRequest).await?;
    let outcome = dispatch
        .run(AgentHookEventPayload::PermissionRequest {
            tool_name: tool_name.to_string(),
            tool_input: tool_input.clone(),
        })
        .await;
    if let Some(reason) = outcome.block_reason.clone() {
        return Some(PermissionRequestHookDecision {
            allow: false,
            message: Some(reason),
        });
    }
    match outcome.permission {
        Some(AgentHookPermissionOutcome::Deny { reason }) => Some(PermissionRequestHookDecision {
            allow: false,
            message: reason,
        }),
        Some(AgentHookPermissionOutcome::Allow { reason }) => Some(PermissionRequestHookDecision {
            allow: true,
            message: reason,
        }),
        None => None,
    }
}

/// PostToolUse hooks run after a tool call completed. Blocking feedback and
/// `additionalContext` are appended to the tool result the model reads.
pub async fn dispatch_post_tool_use(
    facts: NativeHookSessionFacts<'_>,
    tool_name: &str,
    tool_use_id: &str,
    tool_input: &Value,
    tool_response: &Value,
) -> PostToolUseHookDecision {
    let mut decision = PostToolUseHookDecision::default();
    let Some(dispatch) = prepare(facts, AgentHookEvent::PostToolUse).await else {
        return decision;
    };
    let outcome = dispatch
        .run(AgentHookEventPayload::PostToolUse {
            tool_name: tool_name.to_string(),
            tool_use_id: tool_use_id.to_string(),
            tool_input: tool_input.clone(),
            tool_response: tool_response.clone(),
        })
        .await;
    decision.block_reason = outcome.block_reason.clone();
    decision.additional_context = outcome.additional_context.clone();
    decision
}

/// PreCompact hooks observe context compaction (`trigger`: `auto`|`manual`).
pub async fn dispatch_pre_compact(facts: NativeHookSessionFacts<'_>, trigger: &str) {
    if let Some(dispatch) = prepare(facts, AgentHookEvent::PreCompact).await {
        dispatch
            .run(AgentHookEventPayload::PreCompact {
                trigger: trigger.to_string(),
            })
            .await;
    }
}

/// PostCompact hooks observe completed context compaction.
pub async fn dispatch_post_compact(facts: NativeHookSessionFacts<'_>, trigger: &str) {
    if let Some(dispatch) = prepare(facts, AgentHookEvent::PostCompact).await {
        dispatch
            .run(AgentHookEventPayload::PostCompact {
                trigger: trigger.to_string(),
            })
            .await;
    }
}

/// SubagentStart hooks run when a subagent turn begins; plain stdout is
/// returned as model-visible context for the subagent.
pub async fn dispatch_subagent_start(
    facts: NativeHookSessionFacts<'_>,
    agent_id: &str,
    agent_type: &str,
) -> Vec<String> {
    let Some(dispatch) = prepare(facts, AgentHookEvent::SubagentStart).await else {
        return Vec::new();
    };
    let outcome = dispatch
        .run(AgentHookEventPayload::SubagentStart {
            agent_id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
        })
        .await;
    outcome.additional_context.clone()
}

/// SubagentStop hooks run when a subagent turn settles. A blocking decision
/// is recorded (returned) but does not force the subagent to continue.
pub async fn dispatch_subagent_stop(
    facts: NativeHookSessionFacts<'_>,
    agent_id: &str,
    agent_type: &str,
    last_assistant_message: Option<&str>,
) -> Option<String> {
    let dispatch = prepare(facts, AgentHookEvent::SubagentStop).await?;
    let outcome = dispatch
        .run(AgentHookEventPayload::SubagentStop {
            agent_id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
            agent_transcript_path: None,
            stop_hook_active: false,
            last_assistant_message: last_assistant_message.map(str::to_string),
        })
        .await;
    outcome.block_reason.clone()
}

/// Stop hooks run when the agent is about to finish a turn with a final
/// answer. A blocking decision returns the reason; the execution engine
/// injects it and continues the turn.
pub async fn dispatch_stop(
    facts: NativeHookSessionFacts<'_>,
    stop_hook_active: bool,
    last_assistant_message: Option<&str>,
) -> Option<String> {
    let dispatch = prepare(facts, AgentHookEvent::Stop).await?;
    let outcome = dispatch
        .run(AgentHookEventPayload::Stop {
            stop_hook_active,
            last_assistant_message: last_assistant_message.map(str::to_string),
        })
        .await;
    outcome.block_reason.clone()
}

/// SessionEnd hooks run when a session is deleted (`reason: "other"`).
/// Timeouts are capped tightly so deletion never hangs.
pub async fn dispatch_session_end(facts: NativeHookSessionFacts<'_>, reason: &str) {
    pending_session_context().remove(facts.session_id);
    if let Some(dispatch) = prepare(facts, AgentHookEvent::SessionEnd).await {
        dispatch
            .run(AgentHookEventPayload::SessionEnd {
                reason: reason.to_string(),
            })
            .await;
    }
}

/// Drop per-session hook state without dispatching anything.
pub fn clear_session_hook_state(session_id: &str) {
    pending_session_context().remove(session_id);
}

struct PreparedDispatch<'a> {
    engine: Arc<AgentHookEngine>,
    facts: NativeHookSessionFacts<'a>,
    cwd: PathBuf,
}

impl PreparedDispatch<'_> {
    async fn run(&self, event: AgentHookEventPayload) -> AgentHookOutcome {
        let payload = AgentHookPayload {
            common: AgentHookPayloadCommon {
                session_id: self.facts.session_id.to_string(),
                transcript_path: None,
                cwd: self.cwd.to_string_lossy().to_string(),
                model: self.facts.model.to_string(),
                permission_mode: if self.facts.bypass_permissions {
                    AgentHookPermissionMode::BypassPermissions
                } else {
                    AgentHookPermissionMode::Default
                },
                turn_id: self.facts.turn_id.map(str::to_string),
            },
            event,
        };
        let event_name = payload.event();
        let outcome = self.engine.dispatch(&payload, &self.cwd).await;
        for warning in &outcome.warnings {
            warn!("Agent hook warning ({event_name}): {warning}");
        }
        for message in &outcome.system_messages {
            info!("Agent hook message ({event_name}): {message}");
        }
        outcome
    }
}

/// Resolve the hook engine for this event, or `None` when hooks are
/// disabled, unavailable for this workspace, or have no matching rules.
async fn prepare<'a>(
    facts: NativeHookSessionFacts<'a>,
    event: AgentHookEvent,
) -> Option<PreparedDispatch<'a>> {
    if facts.is_remote_workspace {
        debug!(
            "Skipping agent hook dispatch for remote workspace: event={}, session_id={}",
            event, facts.session_id
        );
        return None;
    }
    let config = hooks_config().await;
    if !config.enabled {
        return None;
    }
    let engine = engine_for(facts.workspace_root, config.project_hooks_enabled).await?;
    if !engine.has_rules(event) {
        return None;
    }
    let cwd = facts
        .workspace_root
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    Some(PreparedDispatch { engine, facts, cwd })
}

/// Dot-path of the hook gates inside the settings document. Config paths
/// resolve against the serialized `GlobalConfig`, where `AppConfig` lives
/// under `app`.
pub(crate) const HOOKS_CONFIG_PATH: &str = "app.hooks";

async fn hooks_config() -> AgentHooksConfig {
    match get_global_config_service().await {
        Ok(service) => service
            .get_config::<AgentHooksConfig>(Some(HOOKS_CONFIG_PATH))
            .await
            .unwrap_or_default(),
        // Hosts without an initialized config service keep the defaults:
        // user hooks on, project hooks off.
        Err(_) => AgentHooksConfig::default(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HookFileFingerprint {
    path: PathBuf,
    modified: Option<std::time::SystemTime>,
    len: Option<u64>,
}

fn fingerprint(path: PathBuf) -> HookFileFingerprint {
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => HookFileFingerprint {
            modified: metadata.modified().ok(),
            len: Some(metadata.len()),
            path,
        },
        _ => HookFileFingerprint {
            modified: None,
            len: None,
            path,
        },
    }
}

struct CachedHookEngine {
    engine: Arc<AgentHookEngine>,
    fingerprints: Vec<HookFileFingerprint>,
    project_hooks_enabled: bool,
}

type EngineCache = tokio::sync::Mutex<BTreeMap<Option<PathBuf>, CachedHookEngine>>;

fn engine_cache() -> &'static EngineCache {
    static CACHE: OnceLock<EngineCache> = OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(BTreeMap::new()))
}

fn pending_session_context() -> &'static DashMap<String, Vec<String>> {
    static PENDING: OnceLock<DashMap<String, Vec<String>>> = OnceLock::new();
    PENDING.get_or_init(DashMap::new)
}

/// Hook settings file paths for a workspace, in layer order (user first).
pub(crate) fn hook_settings_paths(
    workspace_root: Option<&Path>,
    project_hooks_enabled: bool,
) -> Vec<(AgentHookScope, PathBuf)> {
    let mut paths = Vec::new();
    if let Ok(path_manager) = try_get_path_manager_arc() {
        paths.push((AgentHookScope::User, path_manager.user_hooks_file()));
        if project_hooks_enabled {
            if let Some(workspace_root) = workspace_root {
                paths.push((
                    AgentHookScope::Project,
                    path_manager.project_hooks_file(workspace_root),
                ));
            }
        }
    }
    paths
}

/// Read each existing hook settings file, in the given layer order, and parse
/// them into one engine. Unreadable or oversized files are skipped with a
/// warning so one bad layer cannot disable the rest.
pub(crate) fn build_engine(paths: &[(AgentHookScope, PathBuf)]) -> AgentHookEngine {
    let mut layers = Vec::new();
    for (scope, path) in paths {
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => {
                if metadata.len() > MAX_HOOKS_FILE_BYTES as u64 {
                    warn!(
                        "Ignoring hook configuration over the {} byte limit: {}",
                        MAX_HOOKS_FILE_BYTES,
                        path.display()
                    );
                    continue;
                }
                match std::fs::read(path) {
                    Ok(bytes) => layers.push(AgentHookSettingsLayer {
                        scope: *scope,
                        source: path.to_string_lossy().to_string(),
                        bytes,
                    }),
                    Err(error) => warn!(
                        "Failed to read hook configuration: path={}, error={}",
                        path.display(),
                        error
                    ),
                }
            }
            _ => {}
        }
    }
    let (settings, issues) = AgentHookSettings::from_layers(&layers);
    for issue in &issues {
        warn!("Agent hook configuration issue: {issue}");
    }
    AgentHookEngine::new(settings)
}

async fn engine_for(
    workspace_root: Option<&Path>,
    project_hooks_enabled: bool,
) -> Option<Arc<AgentHookEngine>> {
    let key = workspace_root.map(Path::to_path_buf);
    let paths = hook_settings_paths(workspace_root, project_hooks_enabled);
    if paths.is_empty() {
        return None;
    }
    let fingerprints = paths
        .iter()
        .map(|(_, path)| fingerprint(path.clone()))
        .collect::<Vec<_>>();
    {
        let cache = engine_cache().lock().await;
        if let Some(cached) = cache.get(&key) {
            if cached.fingerprints == fingerprints
                && cached.project_hooks_enabled == project_hooks_enabled
            {
                return Some(Arc::clone(&cached.engine));
            }
        }
    }

    let engine = Arc::new(build_engine(&paths));
    let mut cache = engine_cache().lock().await;
    if cache.len() >= MAX_CACHED_WORKSPACE_ENGINES && !cache.contains_key(&key) {
        let oldest = cache.keys().next().cloned();
        if let Some(oldest) = oldest {
            cache.remove(&oldest);
        }
    }
    cache.insert(
        key,
        CachedHookEngine {
            engine: Arc::clone(&engine),
            fingerprints,
            project_hooks_enabled,
        },
    );
    Some(engine)
}
