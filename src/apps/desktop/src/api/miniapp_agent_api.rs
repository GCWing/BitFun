//! MiniApp agent bridge API.
//!
//! Lets a MiniApp (gated by the `agent` permission group) run full host agent
//! turns — the complete agent loop with tools (WebSearch/WebFetch/Read/...)
//! and skills — instead of the raw single-call LLM access provided by the
//! `ai` permission group.
//!
//! A run creates or reuses a hidden subagent session (invisible in the session
//! list), owned by `miniapp-agent:{app_id}:{run_id}`, and submits one dialog
//! turn through the standard `DialogScheduler`. Streaming output reaches the
//! MiniApp iframe through the normal `agentic://*` Tauri events, which the
//! web-ui MiniApp bridge filters by session id and forwards into the iframe.

use log::warn;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::api::app_state::AppState;
use bitfun_core::agentic::coordination::{
    ConversationCoordinator, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use bitfun_core::agentic::core::{MessageContent, MessageRole, SessionConfig};
use bitfun_core::miniapp::agent_bridge::{
    agent_run_id_from_request, build_agent_submission_plan, extract_agent_turn_text,
    plan_agent_workspace, require_agent_prompt, require_enabled_agent_permissions,
    validate_reused_session, MiniAppAgentRateLimiter, MiniAppAgentRunRecord,
    MiniAppAgentRunRegistry, MiniAppAgentTurnMessage, MiniAppAgentTurnMessageRole,
    MINIAPP_AGENT_KIND, UNKNOWN_AGENT_RUN_MESSAGE,
};

// ============== Run registry ==============

/// Active/recent agent runs: run_id → record. Used for ownership validation,
/// stale-run cancellation after a webview reload, and turn-text fallback.
static AGENT_RUN_REGISTRY: OnceLock<MiniAppAgentRunRegistry> = OnceLock::new();

/// Per-app agent rate limiter state: app_id → (request_count, window_start_ms).
static AGENT_RATE_LIMITER: OnceLock<MiniAppAgentRateLimiter> = OnceLock::new();

static AGENT_RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

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
pub struct MiniAppAgentRunRequest {
    pub app_id: String,
    /// Full user prompt for the agent turn. The MiniApp owns its own task
    /// protocol; the host only wraps it into a hidden agent session.
    pub prompt: String,
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
        let session = coordinator
            .get_session_manager()
            .get_session(&existing_session_id)
            .ok_or("Unknown MiniApp agent session")?;
        validate_reused_session(
            session.created_by.as_deref(),
            session.config.workspace_path.as_deref(),
            &request.app_id,
            &submission_plan.workspace_path,
        )?;
        if let Some(model_id) = requested_model.as_deref() {
            coordinator
                .update_session_model(&existing_session_id, model_id)
                .await
                .map_err(|e| format!("Failed to update MiniApp agent session model: {}", e))?;
        }
        existing_session_id
    } else {
        // One hidden session per task keeps MiniApp work isolated and out of
        // the visible session list. Follow-up turns may reuse it via sessionId.
        let config = SessionConfig {
            enable_tools: submission_plan.enable_tools,
            safe_mode: true,
            auto_compact: true,
            enable_context_compression: true,
            model_id: requested_model.clone(),
            ..Default::default()
        };
        // Cowork supplies the office skill group and research/file tools when
        // enabled.
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
        session.session_id
    };

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi);

    let outcome = scheduler
        .submit(
            session_id.clone(),
            request.prompt.clone(),
            Some("MiniApp agent run".to_string()),
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
    use super::MiniAppAgentRunRequest;
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
            "appDataWorkspace": "decks/deck-123"
        }))
        .expect("appdata-workspace MiniApp agent request should deserialize");
        assert_eq!(
            request.app_data_workspace.as_deref(),
            Some("decks/deck-123")
        );
        assert!(request.workspace_path.is_none());
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
