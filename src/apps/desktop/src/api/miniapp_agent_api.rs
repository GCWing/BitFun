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
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::api::app_state::AppState;
use bitfun_core::agentic::coordination::{
    ConversationCoordinator, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use bitfun_core::agentic::core::{MessageContent, MessageRole, SessionConfig};

// ============== Run registry ==============

#[derive(Debug, Clone)]
struct MiniAppAgentRunRecord {
    app_id: String,
    session_id: String,
    turn_id: String,
}

/// Active/recent agent runs: run_id → record. Used for ownership validation,
/// stale-run cancellation after a webview reload, and turn-text fallback.
static AGENT_RUN_REGISTRY: OnceLock<Mutex<HashMap<String, MiniAppAgentRunRecord>>> =
    OnceLock::new();

/// Per-app agent rate limiter state: app_id → (request_count, window_start_ms).
static AGENT_RATE_LIMITER: OnceLock<Mutex<HashMap<String, (u32, u64)>>> = OnceLock::new();

static AGENT_RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Cap the per-process registry so completed runs cannot grow it unboundedly.
const AGENT_RUN_REGISTRY_MAX: usize = 256;

fn agent_run_registry() -> &'static Mutex<HashMap<String, MiniAppAgentRunRecord>> {
    AGENT_RUN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn agent_rate_limiter() -> &'static Mutex<HashMap<String, (u32, u64)>> {
    AGENT_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A clean relative subdir contains only normal components: no `..`, no root,
/// no prefix, so joining it onto a base directory can never escape the base.
fn is_clean_relative_subdir(subdir: &str) -> bool {
    let relative = std::path::Path::new(subdir);
    !relative.as_os_str().is_empty()
        && relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

/// Resolve a MiniApp-requested agent workspace inside the app's own appdata
/// directory. The subdir must be a clean relative path (no `..`, no absolute
/// or rooted components) so a MiniApp can never point the agent outside its
/// own storage. The directory is created if missing.
fn resolve_app_data_workspace(
    state: &AppState,
    app_id: &str,
    subdir: &str,
) -> Result<String, String> {
    if !is_clean_relative_subdir(subdir) {
        return Err("appDataWorkspace must be a clean relative path".to_string());
    }
    let relative = std::path::Path::new(subdir);
    let workspace = state
        .miniapp_manager
        .path_manager()
        .miniapp_dir(app_id)
        .join(relative);
    std::fs::create_dir_all(&workspace)
        .map_err(|e| format!("Failed to create MiniApp agent workspace: {}", e))?;
    Ok(workspace.to_string_lossy().to_string())
}

fn check_agent_rate_limit(app_id: &str, rate_limit_per_minute: u32) -> Result<(), String> {
    if rate_limit_per_minute == 0 {
        return Ok(());
    }
    let now = now_ms();
    let window_ms: u64 = 60_000;
    let mut map = agent_rate_limiter()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(app_id.to_string()).or_insert((0, now));
    if now - entry.1 >= window_ms {
        *entry = (1, now);
    } else {
        entry.0 += 1;
        if entry.0 > rate_limit_per_minute {
            return Err(format!(
                "Agent rate limit exceeded: max {} runs/minute",
                rate_limit_per_minute
            ));
        }
    }
    Ok(())
}

fn register_agent_run(record: MiniAppAgentRunRecord) {
    let mut registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if registry.len() >= AGENT_RUN_REGISTRY_MAX {
        // Drop an arbitrary old entry; the registry is a safety net, not a
        // source of truth, so losing the oldest record is acceptable.
        if let Some(key) = registry.keys().next().cloned() {
            registry.remove(&key);
        }
    }
    registry.insert(record.turn_id.clone(), record);
}

fn lookup_agent_run(
    app_id: &str,
    session_id: &str,
    turn_id: &str,
) -> Option<MiniAppAgentRunRecord> {
    let registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry
        .get(turn_id)
        .filter(|record| record.app_id == app_id && record.session_id == session_id)
        .cloned()
}

fn take_agent_runs_for_app(app_id: &str) -> Vec<MiniAppAgentRunRecord> {
    let mut registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let turn_ids: Vec<String> = registry
        .iter()
        .filter(|(_, record)| record.app_id == app_id)
        .map(|(turn_id, _)| turn_id.clone())
        .collect();
    turn_ids
        .into_iter()
        .filter_map(|turn_id| registry.remove(&turn_id))
        .collect()
}

fn remove_agent_run(turn_id: &str) {
    let mut registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry.remove(turn_id);
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
    let agent_perms = app
        .permissions
        .agent
        .clone()
        .ok_or("Agent access is not enabled for this MiniApp")?;
    if !agent_perms.enabled {
        return Err("Agent access is not enabled for this MiniApp".to_string());
    }
    Ok(agent_perms)
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
    if request.prompt.trim().is_empty() {
        return Err("prompt is required".to_string());
    }
    let agent_perms = require_agent_permission(&state, &request.app_id).await?;
    check_agent_rate_limit(
        &request.app_id,
        agent_perms.rate_limit_per_minute.unwrap_or(0),
    )?;

    let workspace_path = if let Some(subdir) = request
        .app_data_workspace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        resolve_app_data_workspace(&state, &request.app_id, subdir)?
    } else {
        request
            .workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("workspacePath is required for MiniApp agent runs")?
            .to_string()
    };

    let run_id = request
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "miniapp-agent-{}-{}",
                request.app_id,
                AGENT_RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
            )
        });
    let owner = format!("miniapp-agent:{}:{}", request.app_id, run_id);
    let session_name = request
        .session_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("MiniApp Agent Run")
        .to_string();

    let requested_session_id = request
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let session_id = if let Some(existing_session_id) = requested_session_id {
        // Reuse a hidden session created by an earlier run of this MiniApp so
        // the new turn shares its context (skills, research, prior outputs).
        let session = coordinator
            .get_session_manager()
            .get_session(&existing_session_id)
            .ok_or("Unknown MiniApp agent session")?;
        let owner_prefix = format!("miniapp-agent:{}:", request.app_id);
        if !session
            .created_by
            .as_deref()
            .is_some_and(|created_by| created_by.starts_with(&owner_prefix))
        {
            return Err("Unknown MiniApp agent session".to_string());
        }
        if session.config.workspace_path.as_deref() != Some(workspace_path.as_str()) {
            return Err("MiniApp agent session workspace does not match this run".to_string());
        }
        existing_session_id
    } else {
        // One hidden session per task keeps MiniApp work isolated and out of
        // the visible session list. Follow-up turns may reuse it via sessionId.
        let enable_tools = request.enable_tools.unwrap_or(true);
        let config = SessionConfig {
            enable_tools,
            safe_mode: true,
            auto_compact: true,
            enable_context_compression: true,
            compression_threshold: 0.65,
            ..Default::default()
        };
        // Cowork supplies the office skill group and research/file tools when
        // enabled.
        let session = coordinator
            .create_hidden_subagent_session_with_workspace(
                None,
                session_name,
                "Cowork".to_string(),
                config,
                workspace_path.clone(),
                Some(owner),
            )
            .await
            .map_err(|e| format!("Failed to create MiniApp agent session: {}", e))?;
        session.session_id
    };

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi)
        .with_skip_tool_confirmation(true);
    let metadata = json!({
        "surface": "miniapp_agent",
        "appId": request.app_id,
        "runId": run_id,
        // Hide AskUserQuestion from the tool manifest; MiniApp iframes have no
        // question UI and would otherwise stall until the channel times out.
        "acp_transport": true,
    });

    let outcome = scheduler
        .submit(
            session_id.clone(),
            request.prompt.clone(),
            Some("MiniApp agent run".to_string()),
            Some(run_id.clone()),
            "Cowork".to_string(),
            Some(workspace_path),
            policy,
            None,
            Some(metadata),
            None,
        )
        .await
        .map_err(|e| format!("Failed to start MiniApp agent turn: {}", e))?;

    let status = match outcome {
        bitfun_core::agentic::coordination::DialogSubmitOutcome::Started { .. } => "started",
        bitfun_core::agentic::coordination::DialogSubmitOutcome::Queued { .. } => "queued",
    };

    register_agent_run(MiniAppAgentRunRecord {
        app_id: request.app_id.clone(),
        session_id: session_id.clone(),
        turn_id: run_id.clone(),
    });

    Ok(MiniAppAgentRunResponse {
        session_id,
        turn_id: run_id.clone(),
        action_run_id: run_id,
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
    if lookup_agent_run(&request.app_id, &request.session_id, &request.turn_id).is_none() {
        return Err("Unknown MiniApp agent run".to_string());
    }
    coordinator
        .cancel_dialog_turn(&request.session_id, &request.turn_id)
        .await
        .map_err(|e| e.to_string())?;
    remove_agent_run(&request.turn_id);
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
    if lookup_agent_run(&request.app_id, &request.session_id, &request.turn_id).is_none() {
        return Err("Unknown MiniApp agent run".to_string());
    }

    let messages = coordinator
        .get_session_manager()
        .get_context_messages(&request.session_id)
        .await
        .map_err(|e| e.to_string())?;
    // Sessions may hold multiple MiniApp turns; only this turn's assistant
    // text is a valid answer for this run. The answer itself may span several
    // assistant messages when the engine continues a truncated stream across
    // rounds ("continue from exactly where you stopped"), so concatenate, in
    // order, every assistant text after this turn's last tool result. The
    // internal reminder user messages between segments do not break the run.
    let turn_messages: Vec<&_> = messages
        .iter()
        .filter(|message| message.metadata.turn_id.as_deref() == Some(request.turn_id.as_str()))
        .collect();
    let answer_start = turn_messages
        .iter()
        .rposition(|message| {
            message.role == MessageRole::Tool
                || matches!(message.content, MessageContent::ToolResult { .. })
        })
        .map_or(0, |index| index + 1);
    let text = turn_messages[answer_start..]
        .iter()
        .filter(|message| message.role == MessageRole::Assistant)
        .filter_map(|message| {
            let text = match &message.content {
                MessageContent::Text(text) => text.as_str(),
                MessageContent::Multimodal { text, .. } => text.as_str(),
                MessageContent::Mixed { text, .. } => text.as_str(),
                MessageContent::ToolResult { .. } => "",
            };
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .collect::<Vec<_>>()
        .concat();

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

    let runs = take_agent_runs_for_app(&request.app_id);
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
    use super::{is_clean_relative_subdir, MiniAppAgentRunRequest};
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
