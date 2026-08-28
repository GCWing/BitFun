//! MiniApp agent bridge API.
//!
//! Lets a MiniApp (gated by the `agent` permission group) run host agent turns
//! instead of the raw single-call LLM access provided by the `ai` permission
//! group. Marketplace runs use a strict tool profile: read-only web research
//! plus Read/Grep confined to bounded app-supplied context files.
//!
//! A run creates or reuses a hidden subagent session (invisible in the session
//! list), owned by `miniapp-agent:{app_id}:{run_id}`, and submits one dialog
//! turn through the standard `DialogScheduler`. Streaming output reaches the
//! MiniApp iframe through the normal `agentic://*` Tauri events, which the
//! web-ui MiniApp bridge filters by session id and forwards into the iframe.

use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::api::app_state::AppState;
use bitfun_core::agentic::coordination::{
    ConversationCoordinator, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use bitfun_core::agentic::core::{MessageContent, MessageRole, Session, SessionConfig};
use bitfun_core::miniapp::agent_bridge::{
    agent_run_id_from_request, build_agent_submission_plan, extract_agent_turn_text,
    plan_agent_workspace, require_agent_prompt, require_enabled_agent_permissions,
    validate_reused_session, MiniAppAgentRateLimiter, MiniAppAgentRunRecord,
    MiniAppAgentRunRegistry, MiniAppAgentSubmissionPlan, MiniAppAgentTurnMessage,
    MiniAppAgentTurnMessageRole, MINIAPP_AGENT_KIND, UNKNOWN_AGENT_RUN_MESSAGE,
    UNKNOWN_AGENT_SESSION_MESSAGE,
};
use bitfun_core::BitFunError;

// ============== Run registry ==============

/// Active/recent agent runs: run_id → record. Used for ownership validation,
/// stale-run cancellation after a webview reload, and turn-text fallback.
static AGENT_RUN_REGISTRY: OnceLock<MiniAppAgentRunRegistry> = OnceLock::new();

/// Per-app agent rate limiter state: app_id → (request_count, window_start_ms).
static AGENT_RATE_LIMITER: OnceLock<MiniAppAgentRateLimiter> = OnceLock::new();

static AGENT_RUN_COUNTER: AtomicU64 = AtomicU64::new(1);
const DEFAULT_MINIAPP_AGENT_DISPLAY_TEXT: &str = "MiniApp agent run";
const MINIAPP_AGENT_CONTEXT_DIR: &str = ".miniapp-context";
const MAX_MINIAPP_AGENT_CONTEXT_FILES: usize = 8;
const MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_MINIAPP_AGENT_CONTEXT_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_MINIAPP_AGENT_CONTEXT_FILE_NAME_BYTES: usize = 128;

fn agent_run_registry() -> &'static MiniAppAgentRunRegistry {
    AGENT_RUN_REGISTRY.get_or_init(MiniAppAgentRunRegistry::default)
}

fn agent_rate_limiter() -> &'static MiniAppAgentRateLimiter {
    AGENT_RATE_LIMITER.get_or_init(MiniAppAgentRateLimiter::default)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn check_agent_rate_limit(app_id: &str, rate_limit_per_minute: u32) -> Result<(), String> {
    agent_rate_limiter().check(app_id, rate_limit_per_minute, now_ms())
}

fn resolve_agent_display_text(display_text: Option<&str>) -> String {
    display_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_MINIAPP_AGENT_DISPLAY_TEXT)
        .to_string()
}

fn is_safe_agent_context_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_MINIAPP_AGENT_CONTEXT_FILE_NAME_BYTES
        && Path::new(name)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && Path::new(name).components().count() == 1
}

fn materialize_agent_context_files(
    workspace_path: &Path,
    app_data_dir: &Path,
    app_data_workspace: Option<&str>,
    context_files: &[MiniAppAgentContextFile],
) -> Result<(), String> {
    if context_files.is_empty() {
        return Ok(());
    }
    if app_data_workspace
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err(
            "contextFiles requires appDataWorkspace so context stays inside MiniApp storage"
                .to_string(),
        );
    }
    if context_files.len() > MAX_MINIAPP_AGENT_CONTEXT_FILES {
        return Err(format!(
            "contextFiles supports at most {} files",
            MAX_MINIAPP_AGENT_CONTEXT_FILES
        ));
    }

    let mut names = HashSet::with_capacity(context_files.len());
    let mut total_bytes = 0usize;
    for file in context_files {
        if !is_safe_agent_context_file_name(&file.name) {
            return Err(format!(
                "Invalid context file name '{}': use one plain file name",
                file.name
            ));
        }
        if !names.insert(file.name.as_str()) {
            return Err(format!("Duplicate context file name: {}", file.name));
        }
        let file_bytes = file.content.len();
        if file_bytes > MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES {
            return Err(format!(
                "Context file '{}' exceeds the {} byte limit",
                file.name, MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES
            ));
        }
        total_bytes = total_bytes
            .checked_add(file_bytes)
            .ok_or_else(|| "contextFiles total size overflowed".to_string())?;
        if total_bytes > MAX_MINIAPP_AGENT_CONTEXT_TOTAL_BYTES {
            return Err(format!(
                "contextFiles exceeds the {} byte total limit",
                MAX_MINIAPP_AGENT_CONTEXT_TOTAL_BYTES
            ));
        }
    }

    let canonical_app_data = std::fs::canonicalize(app_data_dir)
        .map_err(|error| format!("Failed to resolve MiniApp appdata directory: {error}"))?;
    let canonical_workspace = std::fs::canonicalize(workspace_path)
        .map_err(|error| format!("Failed to resolve MiniApp agent workspace: {error}"))?;
    if !canonical_workspace.starts_with(&canonical_app_data) {
        return Err("MiniApp agent workspace escaped app storage".to_string());
    }

    let context_root = canonical_workspace.join(MINIAPP_AGENT_CONTEXT_DIR);
    if std::fs::symlink_metadata(&context_root)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("MiniApp agent context directory must not be a symlink".to_string());
    }
    std::fs::create_dir_all(&context_root)
        .map_err(|error| format!("Failed to create MiniApp agent context directory: {error}"))?;

    let canonical_context_root = std::fs::canonicalize(&context_root)
        .map_err(|error| format!("Failed to resolve MiniApp agent context directory: {error}"))?;
    if !canonical_context_root.starts_with(&canonical_workspace) {
        return Err("MiniApp agent context directory escaped app storage".to_string());
    }

    for file in context_files {
        let target = canonical_context_root.join(&file.name);
        let temp =
            canonical_context_root.join(format!(".{}.{}.tmp", file.name, uuid::Uuid::new_v4()));
        std::fs::write(&temp, file.content.as_bytes()).map_err(|error| {
            format!(
                "Failed to write MiniApp agent context file '{}': {error}",
                file.name
            )
        })?;
        if target.exists()
            || std::fs::symlink_metadata(&target)
                .map(|_| true)
                .unwrap_or(false)
        {
            if let Err(error) = std::fs::remove_file(&target) {
                let _ = std::fs::remove_file(&temp);
                return Err(format!(
                    "Failed to replace MiniApp agent context file '{}': {error}",
                    file.name
                ));
            }
        }
        if let Err(error) = std::fs::rename(&temp, &target) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!(
                "Failed to publish MiniApp agent context file '{}': {error}",
                file.name
            ));
        }
    }
    Ok(())
}

async fn require_agent_permission(
    state: &AppState,
    app_id: &str,
) -> Result<bitfun_core::miniapp::AgentPermissions, String> {
    let app = state
        .miniapp_manager
        .get(app_id)
        .await
        .map_err(|e| e.to_string())?;
    require_enabled_agent_permissions(app.permissions.agent.as_ref())
}

// ============== Request/Response DTOs ==============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentContextFile {
    /// Plain file name placed under the reserved `.miniapp-context` directory.
    pub name: String,
    /// UTF-8 context controlled by the MiniApp and treated as untrusted data by
    /// the receiving Agent prompt.
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentRunRequest {
    pub app_id: String,
    /// Full user prompt for the agent turn. The MiniApp owns its own task
    /// protocol; the host only wraps it into a hidden agent session.
    pub prompt: String,
    /// Optional user-facing text for the shared chat surface. This is kept
    /// separate from `prompt` so a MiniApp can send a structured internal
    /// protocol to the agent while preserving the user's original request in
    /// conversation history. Legacy callers receive a neutral label rather
    /// than exposing their internal prompt.
    #[serde(default)]
    pub display_text: Option<String>,
    /// Optional idempotency key reused as the turn id.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Optional human-readable session name for diagnostics.
    #[serde(default)]
    pub session_name: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    /// Defaults to true for backward compatibility. MiniApps may disable tools
    /// for deterministic render-only turns after a tool-enabled planning turn.
    /// Only applies when a new session is created.
    #[serde(default)]
    pub enable_tools: Option<bool>,
    /// Reuse an existing hidden session created by an earlier run of the same
    /// MiniApp. Later turns then share the session context (loaded skills,
    /// research results, prior outputs), so multi-step tasks load each
    /// resource once and "continue" turns can resume interrupted work.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Relative subdirectory inside the MiniApp's own appdata directory to use
    /// as the agent workspace (created if missing). File-protocol MiniApps use
    /// this so the agent reads/writes project files in app-owned storage
    /// instead of the user's workspace. Must be a clean relative path.
    #[serde(default)]
    pub app_data_workspace: Option<String>,
    /// Optional model selector for the hidden Cowork session (`auto`,
    /// `primary`, `fast`, or a concrete model config id). Applied when the
    /// session is created, and also when an existing session is reused so the
    /// MiniApp can switch models mid-task.
    #[serde(default)]
    pub model: Option<String>,
    /// Bounded app-supplied context materialized under `.miniapp-context` in
    /// the appdata workspace before the turn starts. Marketplace Agents can
    /// Read/Grep only this reserved directory, never the general filesystem.
    #[serde(default)]
    pub context_files: Vec<MiniAppAgentContextFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentRunResponse {
    pub session_id: String,
    pub turn_id: String,
    pub action_run_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentEnsureSessionRequest {
    pub app_id: String,
    /// Rebind a topic to the hidden session that already owns its history.
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub session_name: Option<String>,
    /// Dedicated local workspace inside this MiniApp's appdata directory.
    pub app_data_workspace: String,
    #[serde(default)]
    pub enable_tools: Option<bool>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentEnsureSessionResponse {
    pub session_id: String,
    pub workspace_path: String,
    pub created: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentCancelRequest {
    pub app_id: String,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentTurnTextRequest {
    pub app_id: String,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentTurnTextResponse {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentCancelStaleRunsRequest {
    pub app_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentCancelStaleRunsResponse {
    pub cancelled_runs: u32,
}

// ============== Commands ==============

async fn create_miniapp_agent_session(
    coordinator: &ConversationCoordinator,
    submission_plan: &MiniAppAgentSubmissionPlan,
    requested_model: Option<String>,
) -> Result<String, String> {
    let config = SessionConfig {
        enable_tools: submission_plan.enable_tools,
        safe_mode: true,
        auto_compact: true,
        enable_context_compression: true,
        model_id: requested_model,
        ..Default::default()
    };
    let session = coordinator
        .create_hidden_subagent_session_with_workspace(
            None,
            submission_plan.session_name.clone(),
            MINIAPP_AGENT_KIND.to_string(),
            config,
            submission_plan.workspace_path.clone(),
            Some(submission_plan.owner.clone()),
        )
        .await
        .map_err(|e| format!("Failed to create MiniApp agent session: {}", e))?;
    Ok(session.session_id)
}

async fn load_and_validate_miniapp_agent_session(
    coordinator: &ConversationCoordinator,
    session_id: &str,
    app_id: &str,
    workspace_path: &str,
) -> Result<Option<Session>, String> {
    let session = if let Some(session) = coordinator.get_session_manager().get_session(session_id) {
        session
    } else {
        match coordinator
            .restore_internal_session(Path::new(workspace_path), session_id)
            .await
        {
            Ok(session) => session,
            Err(BitFunError::NotFound(_)) => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "Failed to restore MiniApp agent session: {}",
                    error
                ));
            }
        }
    };

    validate_reused_session(
        session.created_by.as_deref(),
        session.config.workspace_path.as_deref(),
        app_id,
        workspace_path,
    )?;
    Ok(Some(session))
}

/// Align a reused hidden session with the tool policy of the current run.
///
/// `enable_tools` is baked into the session config at creation time, so sessions
/// created by older builds (which disabled tools for marketplace MiniApps) would
/// stay tool-less forever. Marketplace runs are now constrained by the backend
/// research allowlist instead, so the session config is repaired on reuse.
async fn sync_agent_session_tool_enablement(
    coordinator: &ConversationCoordinator,
    session_id: &str,
    submission_plan: &MiniAppAgentSubmissionPlan,
) -> Result<(), String> {
    coordinator
        .update_session_tool_enablement(session_id, submission_plan.enable_tools)
        .await
        .map_err(|e| format!("Failed to update MiniApp agent session tools: {}", e))
}

/// Ensure that one MiniApp topic has a dedicated hidden Agent session before
/// the user opens its floating chat surface. This command intentionally accepts
/// only an appdata-relative workspace, so it remains a local-host capability
/// even while the product is viewing a remote workspace.
#[tauri::command]
pub async fn miniapp_agent_ensure_session(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentEnsureSessionRequest,
) -> Result<MiniAppAgentEnsureSessionResponse, String> {
    let agent_perms = require_agent_permission(&state, &request.app_id).await?;
    let app_data_dir = state
        .miniapp_manager
        .path_manager()
        .miniapp_dir(&request.app_id);
    let workspace_plan = plan_agent_workspace(
        None,
        Some(request.app_data_workspace.as_str()),
        &app_data_dir,
    )?;
    if workspace_plan.create_if_missing {
        std::fs::create_dir_all(&workspace_plan.path)
            .map_err(|e| format!("Failed to create MiniApp agent workspace: {}", e))?;
    }

    let run_sequence = AGENT_RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let run_id = agent_run_id_from_request(&request.app_id, None, run_sequence);
    let submission_plan = build_agent_submission_plan(
        &request.app_id,
        &run_id,
        request.session_name.as_deref(),
        request.session_id.as_deref(),
        &workspace_plan.workspace_path,
        request.enable_tools,
        state
            .miniapp_manager
            .uses_market_strict_runtime(&request.app_id)
            .await,
    );
    let requested_model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let (session_id, created) = if let Some(existing_session_id) =
        submission_plan.requested_session_id.clone()
    {
        if load_and_validate_miniapp_agent_session(
            coordinator.inner().as_ref(),
            &existing_session_id,
            &request.app_id,
            &submission_plan.workspace_path,
        )
        .await?
        .is_some()
        {
            if let Some(model_id) = requested_model.as_deref() {
                coordinator
                    .update_session_model(&existing_session_id, model_id)
                    .await
                    .map_err(|e| format!("Failed to update MiniApp agent session model: {}", e))?;
            }
            sync_agent_session_tool_enablement(
                coordinator.inner().as_ref(),
                &existing_session_id,
                &submission_plan,
            )
            .await?;
            (existing_session_id, false)
        } else {
            check_agent_rate_limit(
                &request.app_id,
                agent_perms.rate_limit_per_minute.unwrap_or(0),
            )?;
            (
                create_miniapp_agent_session(
                    coordinator.inner().as_ref(),
                    &submission_plan,
                    requested_model,
                )
                .await?,
                true,
            )
        }
    } else {
        check_agent_rate_limit(
            &request.app_id,
            agent_perms.rate_limit_per_minute.unwrap_or(0),
        )?;
        (
            create_miniapp_agent_session(
                coordinator.inner().as_ref(),
                &submission_plan,
                requested_model,
            )
            .await?,
            true,
        )
    };

    Ok(MiniAppAgentEnsureSessionResponse {
        session_id,
        workspace_path: workspace_plan.workspace_path,
        created,
    })
}

/// Start a full agent turn for a MiniApp inside a hidden subagent session.
#[tauri::command]
pub async fn miniapp_agent_run(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    scheduler: State<'_, Arc<DialogScheduler>>,
    request: MiniAppAgentRunRequest,
) -> Result<MiniAppAgentRunResponse, String> {
    require_agent_prompt(&request.prompt)?;
    let agent_perms = require_agent_permission(&state, &request.app_id).await?;
    check_agent_rate_limit(
        &request.app_id,
        agent_perms.rate_limit_per_minute.unwrap_or(0),
    )?;

    let app_data_dir = state
        .miniapp_manager
        .path_manager()
        .miniapp_dir(&request.app_id);
    let workspace_plan = plan_agent_workspace(
        request.workspace_path.as_deref(),
        request.app_data_workspace.as_deref(),
        &app_data_dir,
    )?;
    if workspace_plan.create_if_missing {
        std::fs::create_dir_all(&workspace_plan.path)
            .map_err(|e| format!("Failed to create MiniApp agent workspace: {}", e))?;
    }
    materialize_agent_context_files(
        &workspace_plan.path,
        &app_data_dir,
        request.app_data_workspace.as_deref(),
        &request.context_files,
    )?;
    let workspace_path = workspace_plan.workspace_path.clone();
    let run_sequence = if request
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        0
    } else {
        AGENT_RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
    };
    let run_id =
        agent_run_id_from_request(&request.app_id, request.run_id.as_deref(), run_sequence);
    let submission_plan = build_agent_submission_plan(
        &request.app_id,
        &run_id,
        request.session_name.as_deref(),
        request.session_id.as_deref(),
        &workspace_path,
        request.enable_tools,
        state
            .miniapp_manager
            .uses_market_strict_runtime(&request.app_id)
            .await,
    );

    let requested_model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let session_id = if let Some(existing_session_id) = submission_plan.requested_session_id.clone()
    {
        // Reuse a hidden session created by an earlier run of this MiniApp so
        // the new turn shares its context (skills, research, prior outputs).
        load_and_validate_miniapp_agent_session(
            coordinator.inner().as_ref(),
            &existing_session_id,
            &request.app_id,
            &submission_plan.workspace_path,
        )
        .await?
        .ok_or_else(|| UNKNOWN_AGENT_SESSION_MESSAGE.to_string())?;
        if let Some(model_id) = requested_model.as_deref() {
            coordinator
                .update_session_model(&existing_session_id, model_id)
                .await
                .map_err(|e| format!("Failed to update MiniApp agent session model: {}", e))?;
        }
        sync_agent_session_tool_enablement(
            coordinator.inner().as_ref(),
            &existing_session_id,
            &submission_plan,
        )
        .await?;
        existing_session_id
    } else {
        // One hidden session per task keeps MiniApp work isolated and out of
        // the visible session list. Follow-up turns may reuse it via sessionId.
        create_miniapp_agent_session(
            coordinator.inner().as_ref(),
            &submission_plan,
            requested_model.clone(),
        )
        .await?
    };

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi);
    let display_text = resolve_agent_display_text(request.display_text.as_deref());

    let outcome = scheduler
        .submit(
            session_id.clone(),
            request.prompt.clone(),
            Some(display_text),
            Some(submission_plan.run_id.clone()),
            MINIAPP_AGENT_KIND.to_string(),
            Some(submission_plan.workspace_path.clone()),
            None,
            None,
            policy,
            None,
            Some(submission_plan.metadata.clone()),
            None,
        )
        .await
        .map_err(|e| format!("Failed to start MiniApp agent turn: {}", e))?;

    let status = match outcome {
        bitfun_core::agentic::coordination::DialogSubmitOutcome::Started { .. } => "started",
        bitfun_core::agentic::coordination::DialogSubmitOutcome::Queued { .. } => "queued",
    };

    agent_run_registry().register(MiniAppAgentRunRecord {
        app_id: request.app_id.clone(),
        session_id: session_id.clone(),
        turn_id: submission_plan.run_id.clone(),
    });

    Ok(MiniAppAgentRunResponse {
        session_id,
        turn_id: submission_plan.run_id.clone(),
        action_run_id: submission_plan.run_id,
        status: status.to_string(),
    })
}

/// Cancel a running MiniApp agent turn.
#[tauri::command]
pub async fn miniapp_agent_cancel(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentCancelRequest,
) -> Result<(), String> {
    require_agent_permission(&state, &request.app_id).await?;
    if agent_run_registry()
        .lookup(&request.app_id, &request.session_id, &request.turn_id)
        .is_none()
    {
        return Err(UNKNOWN_AGENT_RUN_MESSAGE.to_string());
    }
    coordinator
        .cancel_dialog_turn(&request.session_id, &request.turn_id)
        .await
        .map_err(|e| e.to_string())?;
    agent_run_registry().remove(&request.turn_id);
    Ok(())
}

/// Read the assistant text of a (completed) MiniApp agent turn from the live
/// in-memory session. Used by MiniApps as a fallback when streaming was
/// interrupted (for example a webview reload during generation).
#[tauri::command]
pub async fn miniapp_agent_turn_text(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentTurnTextRequest,
) -> Result<MiniAppAgentTurnTextResponse, String> {
    require_agent_permission(&state, &request.app_id).await?;
    if agent_run_registry()
        .lookup(&request.app_id, &request.session_id, &request.turn_id)
        .is_none()
    {
        return Err(UNKNOWN_AGENT_RUN_MESSAGE.to_string());
    }

    let messages = coordinator
        .get_session_manager()
        .get_context_messages(&request.session_id)
        .await
        .map_err(|e| e.to_string())?;
    let turn_messages: Vec<MiniAppAgentTurnMessage> = messages
        .iter()
        .map(|message| {
            let role = if message.role == MessageRole::Assistant {
                MiniAppAgentTurnMessageRole::Assistant
            } else if message.role == MessageRole::Tool {
                MiniAppAgentTurnMessageRole::Tool
            } else {
                MiniAppAgentTurnMessageRole::Other
            };
            let text = match &message.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::Multimodal { text, .. } => text.clone(),
                MessageContent::Mixed { text, .. } => text.clone(),
                MessageContent::ToolResult { .. } => String::new(),
            };
            MiniAppAgentTurnMessage {
                turn_id: message.metadata.turn_id.clone(),
                role,
                is_tool_result: matches!(message.content, MessageContent::ToolResult { .. }),
                text,
            }
        })
        .collect();
    let text = extract_agent_turn_text(&turn_messages, &request.turn_id);

    Ok(MiniAppAgentTurnTextResponse { text })
}

/// Cancel every tracked agent run for the given MiniApp. Called by the app on
/// startup/recovery so webview reloads do not leave orphaned agent turns.
#[tauri::command]
pub async fn miniapp_agent_cancel_stale_runs(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentCancelStaleRunsRequest,
) -> Result<MiniAppAgentCancelStaleRunsResponse, String> {
    require_agent_permission(&state, &request.app_id).await?;

    let runs = agent_run_registry().take_for_app(&request.app_id);
    let mut cancelled = 0u32;
    for run in runs {
        match coordinator
            .cancel_dialog_turn(&run.session_id, &run.turn_id)
            .await
        {
            Ok(()) => cancelled += 1,
            Err(error) => {
                // Completed turns fail to cancel; that is the expected steady state.
                warn!(
                    "MiniApp agent stale-run cancel skipped: app_id={}, session_id={}, turn_id={}, error={}",
                    run.app_id, run.session_id, run.turn_id, error
                );
            }
        }
    }

    Ok(MiniAppAgentCancelStaleRunsResponse {
        cancelled_runs: cancelled,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        materialize_agent_context_files, resolve_agent_display_text, MiniAppAgentContextFile,
        MiniAppAgentEnsureSessionRequest, MiniAppAgentRunRequest,
        DEFAULT_MINIAPP_AGENT_DISPLAY_TEXT, MINIAPP_AGENT_CONTEXT_DIR,
    };
    use bitfun_core::miniapp::agent_bridge::is_clean_relative_subdir;
    use serde_json::json;

    #[test]
    fn miniapp_agent_run_request_keeps_tool_enablement_backward_compatible() {
        let legacy: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "plan",
            "workspacePath": "/tmp/workspace"
        }))
        .expect("legacy MiniApp agent request should deserialize");
        assert!(legacy.enable_tools.unwrap_or(true));
        assert!(legacy.session_id.is_none());
        assert!(legacy.display_text.is_none());
        assert!(legacy.context_files.is_empty());

        let render: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "render",
            "workspacePath": "/tmp/workspace",
            "enableTools": false
        }))
        .expect("render-only MiniApp agent request should deserialize");
        assert_eq!(render.enable_tools, Some(false));
    }

    #[test]
    fn miniapp_agent_run_request_accepts_session_reuse() {
        let follow_up: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "render slide 2",
            "workspacePath": "/tmp/workspace",
            "sessionId": "session-1"
        }))
        .expect("session-reuse MiniApp agent request should deserialize");
        assert_eq!(follow_up.session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn miniapp_agent_run_request_accepts_app_data_workspace() {
        let request: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "plan a deck",
            "appDataWorkspace": "decks/deck-123",
            "contextFiles": [{
                "name": "summary.json",
                "content": "{\"topic\":\"quarterly review\"}"
            }]
        }))
        .expect("appdata-workspace MiniApp agent request should deserialize");
        assert_eq!(
            request.app_data_workspace.as_deref(),
            Some("decks/deck-123")
        );
        assert!(request.workspace_path.is_none());
        assert_eq!(request.context_files.len(), 1);
        assert_eq!(request.context_files[0].name, "summary.json");
    }

    #[test]
    fn miniapp_agent_run_materializes_bounded_context_inside_appdata_workspace() {
        let temp = tempfile::tempdir().expect("create context workspace");
        let app_data = temp.path().join("app-data");
        let workspace = app_data.join("chat");
        std::fs::create_dir_all(&workspace).expect("create appdata workspace");
        let files = vec![
            MiniAppAgentContextFile {
                name: "stocks.ndjson".to_string(),
                content: "{\"code\":\"688256\"}\n".to_string(),
            },
            MiniAppAgentContextFile {
                name: "summary.json".to_string(),
                content: "{\"market\":\"CN\"}".to_string(),
            },
        ];

        materialize_agent_context_files(&workspace, &app_data, Some("chat"), &files)
            .expect("materialize context files");

        assert_eq!(
            std::fs::read_to_string(
                workspace
                    .join(MINIAPP_AGENT_CONTEXT_DIR)
                    .join("stocks.ndjson")
            )
            .unwrap(),
            "{\"code\":\"688256\"}\n"
        );
    }

    #[test]
    fn miniapp_agent_context_files_reject_paths_and_user_workspaces() {
        let temp = tempfile::tempdir().expect("create context workspace");
        let app_data = temp.path().join("app-data");
        let workspace = app_data.join("chat");
        std::fs::create_dir_all(&workspace).expect("create appdata workspace");
        let escaped = vec![MiniAppAgentContextFile {
            name: "../storage.json".to_string(),
            content: "secret".to_string(),
        }];
        assert!(
            materialize_agent_context_files(&workspace, &app_data, Some("chat"), &escaped)
                .unwrap_err()
                .contains("Invalid context file name")
        );

        let valid = vec![MiniAppAgentContextFile {
            name: "summary.json".to_string(),
            content: "{}".to_string(),
        }];
        assert!(
            materialize_agent_context_files(&workspace, &app_data, None, &valid)
                .unwrap_err()
                .contains("requires appDataWorkspace")
        );
    }

    #[cfg(unix)]
    #[test]
    fn miniapp_agent_context_files_reject_symlinked_appdata_workspaces() {
        let temp = tempfile::tempdir().expect("create context workspace");
        let app_data = temp.path().join("app-data");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&app_data).expect("create appdata");
        std::fs::create_dir_all(&outside).expect("create outside directory");
        std::os::unix::fs::symlink(&outside, app_data.join("chat"))
            .expect("create workspace symlink");
        let files = vec![MiniAppAgentContextFile {
            name: "summary.json".to_string(),
            content: "{}".to_string(),
        }];

        let error = materialize_agent_context_files(
            &app_data.join("chat"),
            &app_data,
            Some("chat"),
            &files,
        )
        .expect_err("symlinked workspace must not escape appdata");
        assert!(error.contains("workspace escaped app storage"));
        assert!(!outside.join(MINIAPP_AGENT_CONTEXT_DIR).exists());
    }

    #[test]
    fn miniapp_agent_run_request_accepts_model_selector() {
        let legacy: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "plan"
        }))
        .expect("legacy MiniApp agent request should deserialize without model");
        assert!(legacy.model.is_none());

        let with_model: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "plan",
            "model": "fast"
        }))
        .expect("MiniApp agent request should accept model");
        assert_eq!(with_model.model.as_deref(), Some("fast"));
    }

    #[test]
    fn miniapp_agent_run_request_accepts_user_facing_display_text() {
        let request: MiniAppAgentRunRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "prompt": "internal structured prompt",
            "displayText": "随便做几页测试页"
        }))
        .expect("MiniApp agent request should accept display text");

        assert_eq!(request.display_text.as_deref(), Some("随便做几页测试页"));
        assert_eq!(
            resolve_agent_display_text(request.display_text.as_deref()),
            "随便做几页测试页"
        );
    }

    #[test]
    fn miniapp_agent_display_text_uses_a_safe_legacy_fallback() {
        assert_eq!(
            resolve_agent_display_text(None),
            DEFAULT_MINIAPP_AGENT_DISPLAY_TEXT
        );
        assert_eq!(
            resolve_agent_display_text(Some("  ")),
            DEFAULT_MINIAPP_AGENT_DISPLAY_TEXT
        );
        assert_eq!(
            resolve_agent_display_text(Some("  Build a deck  ")),
            "Build a deck"
        );
    }

    #[test]
    fn miniapp_agent_ensure_session_request_is_appdata_scoped() {
        let request: MiniAppAgentEnsureSessionRequest = serde_json::from_value(json!({
            "appId": "builtin-ppt-live",
            "sessionId": "session-1",
            "sessionName": "PPT Live",
            "appDataWorkspace": "decks/deck-123",
            "model": "primary"
        }))
        .expect("ensure-session request should deserialize");

        assert_eq!(request.session_id.as_deref(), Some("session-1"));
        assert_eq!(request.session_name.as_deref(), Some("PPT Live"));
        assert_eq!(request.app_data_workspace, "decks/deck-123");
        assert_eq!(request.model.as_deref(), Some("primary"));
    }

    #[test]
    fn app_data_workspace_subdir_must_stay_inside_app_storage() {
        assert!(is_clean_relative_subdir("decks/deck-123"));
        assert!(is_clean_relative_subdir("decks"));
        assert!(!is_clean_relative_subdir(""));
        assert!(!is_clean_relative_subdir("/etc"));
        assert!(!is_clean_relative_subdir("../outside"));
        assert!(!is_clean_relative_subdir("decks/../../outside"));
        assert!(!is_clean_relative_subdir("./decks"));
    }
}
